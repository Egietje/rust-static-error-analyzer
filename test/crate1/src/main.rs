mod mod1;

use mod1::fn3 as fn4;
use std::thread::spawn;

fn main() {
    fn1();
    fn1();
    fn2();

    let handle = spawn(res);
    let join = handle.join();
    let res = join.expect("Thread panicked!");

    let x: usize = 1;
    match x {
        _ => {}
    }
}

const fn test() -> usize {
    1
}

fn fn1() -> Res<()> {
    main();
    fn4();

    Ok(())
}

fn fn2() {
    mod1::fn3();
    fn1();
    main();
}

fn res() -> Result<(), ()> {
    Ok(())
}

fn unreachable() {}

type Res<T> = Result<T, ()>;
