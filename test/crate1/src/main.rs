mod mod1;

fn main() {
    propagate();
    other();
}

fn other() {}

fn result() -> Result<(), MyError> {
    Ok(())
}

struct MyError;

impl From<MyError> for () {
    fn from(value: MyError) -> Self {
        ()
    }
}

fn propagate() -> Result<(), ()> {
    let x = result()?;

    let y = result();

    let z = y?;

    Ok(())
}
