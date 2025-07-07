macro_rules! sget {
    (struct) => {
        (Value::get_struct, Value::get_ref_structure, "Structure")
    };
    (float) => {
        (Value::get_float, Value::get_ref_float, "Float")
    };
    (num) => {
        (Value::get_num, Value::get_ref_num, "Number")
    };
    (str) => {
        (Value::get_str, Value::get_ref_str, "String")
    };
    (bool) => {
        (Value::get_bool, Value::get_ref_bool, "Boolean")
    };
    (arr) => {
        (Value::get_arr, Value::get_ref_arr, "Array")
    };
    (map) => {
        (Value::get_map, Value::get_ref_map, "Map")
    };
    (result) => {
        (Value::get_result, Value::get_ref_result, "Result")
    };
    (option) => {
        (Value::get_option, Value::get_ref_option, "Option")
    };
    (closure) => {
        (Value::get_closure, Value::get_ref_closure, "Closure")
    };
}
pub(crate) use sget;

macro_rules! stack_pop {
    (($stack:expr) -> $type:ident as $this_arg:literal for $fn_name:expr) => {
        $stack
            .pop_this(sget!($type).0)
            .ok_or(RuntimeErrorKind::MissingValueForBuiltin{
                for_fn: $fn_name.to_owned(),
                args: format!( "[{}: {}]", $this_arg, sget!($type).2 ),
                this_arg: $this_arg,
            })
            .and_then(|got_v|{
                got_v.map_err(|got|{
                    RuntimeErrorKind::WrongTypeForBuiltin {
                        for_fn: $fn_name.to_owned(),
                        args: stringify!( [ $this_arg: $type ] ),
                        this_arg: $this_arg,
                        got: Box::new(got),
                        expected: sget!($type).2
                    }
                })
            })
    };
    (($stack:expr) -> $type:ident? as $this_arg:literal for $fn_name:expr) => {
        $stack
            .pop_this(sget!($type).0)
            .map(|got_v|{
                got_v.map_err(|got|{
                    RuntimeErrorKind::WrongTypeForBuiltin {
                        for_fn: $fn_name.to_owned(),
                        args: stringify!( [ $this_arg: $ty ] ),
                        this_arg: $this_arg,
                        got: Box::new(got),
                        expected: sget!($type).2
                    }
                })
            }).transpose()
    };
    (=($stack:expr) -> $type:ident? as $this_arg:literal for $fn_name:expr) => {
        $stack
            .pop_this(sget!($type).0)
    };
    (($stack:expr) -> * as $this_arg:literal for $fn_name:expr) => {
        $stack
            .pop()
            .ok_or(RuntimeErrorKind::MissingValueForBuiltin{
                for_fn: $fn_name.to_owned(),
                args: format!( "[{}]", $this_arg ),
                this_arg: $this_arg,
            })
    };
    (($stack:expr) -> &$type:ident as $this_arg:literal for $fn_name:expr) => {
        $stack
            .peek_this(sget!($type).1)
            .ok_or(RuntimeErrorKind::MissingValueForBuiltin{
                for_fn: $fn_name.to_owned(),
                args: format!( "[{}: {}]", $this_arg, sget!($type).2 ),
                this_arg: $this_arg,
            })
            .and_then(|got_v|{
                got_v.map_err(|got|{
                    RuntimeErrorKind::WrongTypeForBuiltin {
                        for_fn: $fn_name.to_owned(),
                        args: stringify!( [ $this_arg: $ty ] ),
                        this_arg: $this_arg,
                        got: Box::new(got.clone()),
                        expected: sget!($type).2
                    }
                })
            })
    };
}
pub(crate) use stack_pop;
