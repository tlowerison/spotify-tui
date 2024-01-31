use ::darling::{ast::Fields, FromDeriveInput, FromField, FromMeta, FromVariant};
use ::proc_macro2::TokenStream;
use ::quote::{format_ident, quote};
use ::syn::{parse::Error, parse2, parse_str, spanned::Spanned};

#[derive(Clone, Debug, FromDeriveInput)]
#[darling(
    attributes(to_static),
    forward_attrs(allow, doc, cfg),
    supports(struct_named, struct_tuple, enum_any)
)]
struct ToStaticOpts {
    // forwarded fields
    ident: ::syn::Ident,
    data: darling::ast::Data<ToStaticVariant, ToStaticField>,
}

#[derive(Clone, Debug, FromField)]
#[darling(attributes(to_static))]
struct ToStaticField {
    map: Option<ToStaticFieldMap>,
    // forwarded fields
    ident: Option<::syn::Ident>,
    ty: ::syn::Type,
}

#[derive(Clone, Debug)]
enum ToStaticFieldMap {
    FormatExpr(String),
    Path(::syn::Path),
}

#[derive(Clone, Debug, FromVariant)]
struct ToStaticVariant {
    pub ident: ::syn::Ident,
    pub fields: ::darling::ast::Fields<ToStaticField>,
}

impl ToStaticField {
    fn to_static(&self, ident: &::syn::Ident, _ty: &::syn::Type) -> Result<TokenStream, Error> {
        self.map
            .as_ref()
            .map(|map| map.to_static(ident))
            .unwrap_or_else(|| Ok(quote!(#ident.to_static())))
    }
}

impl ToStaticFieldMap {
    fn to_static(&self, ident: &::syn::Ident) -> Result<TokenStream, Error> {
        match self {
            Self::FormatExpr(format_expr) => {
                parse_str(&format_expr.replace("{.0}", &ident.to_string()))
            }
            Self::Path(path) => Ok(quote!(#path(#ident))),
        }
    }
}

impl FromMeta for ToStaticFieldMap {
    fn from_expr(expr: &syn::Expr) -> Result<Self, darling::Error> {
        Ok(match expr {
            ::syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => Self::FormatExpr(lit_str.value()),
            ::syn::Expr::Path(path) => Self::Path(path.path.clone()),
            _ => {
                return Err(
                    darling::Error::custom("expected function path or format string")
                        .with_span(&expr.span()),
                )
            }
        })
    }
}

pub fn derive_to_static(tokens: TokenStream) -> Result<TokenStream, Error> {
    let ast: ::syn::DeriveInput = parse2(tokens)?;

    let ToStaticOpts {
        ident: data_ident,
        data,
        ..
    } = ToStaticOpts::from_derive_input(&ast)?;

    let receiver_ident = format_ident!("__receiver");

    let body = if data.is_enum() {
        let match_arms = data
            .take_enum()
            .unwrap()
            .into_iter()
            .map(|ToStaticVariant { ident, fields }| {
                map_fields(parse2(quote!(#data_ident::#ident)).unwrap(), fields)
                    .map(|(scope, instantiation)| quote!(#scope => #instantiation))
            })
            .collect::<Result<Vec<_>, _>>()?;
        quote!(match #receiver_ident { #(#match_arms),* })
    } else if data.is_struct() {
        let fields = data.take_struct().unwrap();
        let (scope, instantiation) = map_fields(parse2(quote!(#data_ident)).unwrap(), fields)?;
        quote!(let #scope = #receiver_ident; #instantiation)
    } else {
        quote!(#data_ident)
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let mut owned_generics = ast.generics.clone();
    owned_generics.params = owned_generics
        .params
        .into_iter()
        .map(|param| match param {
            ::syn::GenericParam::Lifetime(lt_param) => {
                ::syn::GenericParam::Lifetime(::syn::LifetimeParam {
                    lifetime: parse2(quote!('static)).unwrap(),
                    ..lt_param
                })
            }
            _ => param,
        })
        .collect();

    let (_, owned_ty_generics, _) = owned_generics.split_for_impl();

    let tokens = quote!(
        impl #impl_generics ::spotify_tui_util::ToStatic for #data_ident #ty_generics #where_clause {
            type Static = #data_ident #owned_ty_generics;
            fn to_static(self) -> Self::Static {
                let #receiver_ident = self;
                #body
            }
        }
    );

    Ok(tokens)
}

fn map_fields(
    scope_path: ::syn::Path,
    fields: Fields<ToStaticField>,
) -> Result<(TokenStream, TokenStream), Error> {
    let accessors = field_accessors(&fields.fields);
    let idents = field_idents(&fields.fields);
    let mut scope = fields_wrapped(fields.style, quote!(#(#idents),*));
    scope = quote!(#scope_path #scope);

    let fields_to_static = fields
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| field.to_static(&idents[i], &field.ty))
        .collect::<Result<Vec<_>, Error>>()?;

    let instantiation = quote!(#scope_path { #(#accessors: #fields_to_static),* });

    Ok((scope, instantiation))
}

fn fields_wrapped(style: ::darling::ast::Style, tokens: TokenStream) -> TokenStream {
    match style {
        ::darling::ast::Style::Unit => quote!(),
        ::darling::ast::Style::Struct => quote!({ #tokens }),
        ::darling::ast::Style::Tuple => quote!(( #tokens )),
    }
}

fn field_accessors<'a>(fields: impl IntoIterator<Item = &'a ToStaticField>) -> Vec<TokenStream> {
    fields
        .into_iter()
        .enumerate()
        .map(|(i, field)| {
            field
                .ident
                .clone()
                .map(|ident| quote!(#ident))
                .unwrap_or_else(|| parse_str(&i.to_string()).unwrap())
        })
        .collect()
}

fn field_idents<'a>(fields: impl IntoIterator<Item = &'a ToStaticField>) -> Vec<::syn::Ident> {
    fields
        .into_iter()
        .enumerate()
        .map(|(i, field)| field.ident.clone().unwrap_or_else(|| format_ident!("_{i}")))
        .collect()
}
