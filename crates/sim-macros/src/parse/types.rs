use super::ParsedType;
use syn::Type;

pub(super) fn parse_type(ty: &Type) -> syn::Result<ParsedType> {
    match ty {
        Type::Path(path) if path.path.is_ident("f64") => Ok(ParsedType::F64),
        Type::Path(path) if path.path.is_ident("bool") => Ok(ParsedType::Bool),
        Type::Path(path) => {
            let ident = path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.clone())
                .ok_or_else(|| syn::Error::new_spanned(path, "expected type path"))?;
            match ident.to_string().as_str() {
                "String" => Ok(ParsedType::String),
                "Symbol" => Ok(ParsedType::Symbol),
                "Expr" => Ok(ParsedType::Expr),
                _ => Ok(ParsedType::OwnedClass(ident)),
            }
        }
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Path(path) => {
                let ident = path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.clone())
                    .ok_or_else(|| syn::Error::new_spanned(path, "expected reference type path"))?;
                match ident.to_string().as_str() {
                    "str" | "String" | "Symbol" | "Expr" | "f64" | "bool" => {
                        Err(syn::Error::new_spanned(
                            path,
                            "borrowed builtin types are not supported here",
                        ))
                    }
                    _ => Ok(ParsedType::RefClass(ident)),
                }
            }
            other => Err(syn::Error::new_spanned(other, "unsupported reference type")),
        },
        other => Err(syn::Error::new_spanned(
            other,
            "unsupported type in proc macro lib",
        )),
    }
}
