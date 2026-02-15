use conduit::{input, try_pipeline};

#[allow(unused_imports)]
use example::nodes::*;

/// Arithmetic example: converts Celsius to Fahrenheit and Kelvin using DSL expressions.
fn main() {
    let input = input! { celsius: 25.0 };

    let output: Result<(f64, f64), _> = try_pipeline! { input, r#"
        -> celsius
        
        store _ {
            fahrenheit <- (celsius * 9 / 5 + 32)
            kelvin <- (celsius + 273.15)
        }
        
        <- store::fahrenheit, store::kelvin
    "# };

    match output {
        Ok((fahrenheit, kelvin)) => {
            println!("Celsius: {:.2}°C", 25.0);
            println!("Fahrenheit: {:.2}°F", fahrenheit);
            println!("Kelvin: {:.2}K", kelvin);
        }
        Err(error) => println!("Pipeline failed: {}", error),
    }
}
