fn main() {
    other();
}

fn result() -> Result<(), MyError> {
    other()
}

fn other() -> Result<(), MyError> {
    result()
}

#[derive(Debug)]
struct MyError;