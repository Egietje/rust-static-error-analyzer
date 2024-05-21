mod mod1;

use mod1::fn3 as fn4;
use std::thread::spawn;

fn main() {
    fn1();
    fn2();

    let handle = spawn(res);
    let res = handle.join();
    res.unwrap().unwrap();
}

fn fn1() {
    main();
    fn4();
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
