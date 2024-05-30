mod mod1;

fn main() {
    propagate();
}

fn result() -> Result<(), ()> {
    Ok(())
}

fn propagate() -> Result<(), ()> {
    let x = result().map_err(|_| ())?;

    let y = result().map_err(|_| ());

    let z = y?;

    Ok(())
}
