#[macro_export]
macro_rules! test_helper {
    ( $cmd:ident, $data:ident, $name:ident, $type:ident, $size:expr, $( ( $method:ident, $val:expr ) ,)*) => {{
        {
            assert!($type::new(&[]).is_err());
            let res = $type::new(&$data[..]).unwrap();
            assert_eq!(res.len(), $size);
            $(
                assert_eq!(res.$method(), $val);
            )*
        }
    }};

    ( $cmd:ident, $name:ident, $type:ident ) => {{
        {
            let data = [];
            let mc = $cmd::$name($type::new(&data[..]));
            assert_eq!(mc.len(), 0);
        }
    }};
}
