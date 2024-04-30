fn test_alias_actual() -> Res<()> {
    Res::Ok(())
}

fn test_alias_other() -> OtherType<()> {
    Some(())
}

fn test_actual() -> Result<(), ()> {
    Ok(())
}

fn test_other() -> Option<()> {
    Some(())
}

fn test_default() {

}

fn test_module_actual() -> core::result::Result<(), ()> {
    Ok(())
}

fn test_module_actual_std() -> std::result::Result<(), ()> {
    std::result::Result::Ok(())
}

fn test_module_other() -> core::option::Option<()> {
    Some(())
}

fn main() {
    panic!();
}

type Res<T> = Result<T, ()>;

type OtherType<T> = Option<T>;
