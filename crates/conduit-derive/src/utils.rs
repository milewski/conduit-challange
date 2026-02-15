use syn::{GenericArgument, PathArguments, Type};

pub(crate) fn extract_emitter_event_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Emitter" {
        return None;
    }

    let PathArguments::AngleBracketed(generic_arguments) = &last_segment.arguments else {
        return None;
    };

    let syn::GenericArgument::Type(event_type) = generic_arguments.args.first()? else {
        return None;
    };

    Some(event_type.clone())
}

pub(crate) fn extract_result_types(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(generic_arguments) = &last_segment.arguments else {
        return None;
    };

    let mut generic_arguments = generic_arguments.args.iter();
    let syn::GenericArgument::Type(ok_type) = generic_arguments.next()? else {
        return None;
    };
    let syn::GenericArgument::Type(error_type) = generic_arguments.next()? else {
        return None;
    };

    Some((ok_type, error_type))
}

pub(crate) fn extract_option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Option" {
        return None;
    }

    let PathArguments::AngleBracketed(generic_arguments) = &last_segment.arguments else {
        return None;
    };

    let syn::GenericArgument::Type(inner_type) = generic_arguments.args.first()? else {
        return None;
    };

    Some(inner_type)
}

pub(crate) fn to_snake_case(name: &str) -> String {
    let mut snake_case = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() {
            if index != 0 {
                snake_case.push('_');
            }
            for lowercase_character in character.to_lowercase() {
                snake_case.push(lowercase_character);
            }
        } else {
            snake_case.push(character);
        }
    }
    snake_case
}
