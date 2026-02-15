use conduit::pipeline;

#[allow(unused_imports)]
use example::nodes::*;

fn main() {
    let pipeline = include_str!("./workflows/events.conduit");
    let (name, age): (String, u32) = pipeline!(pipeline);

    println!("Answer: {}, {}", name, age);
}
