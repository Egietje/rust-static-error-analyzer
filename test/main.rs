mod mod1;

use mod1::fn3 as fn4;

fn main() {
    fn2();
    fn1();
    fn1();
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

fn unreachable() {}
