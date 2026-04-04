#[macro_export]
macro_rules! storage {
    ($ios:expr, $storage:expr, [ $( ($key:expr, $path:expr, $type:ty, $f1:expr, $f2:expr) ),* $(,)? ]) => {
        $(
            let s_clone = $storage.clone();
            $ios.insert::<$type>(
                $key.to_string(),
                $path,
                Box::new($f1),
                Box::new(move |file| {
                    let v = $f2;
                    s_clone.save(file, &v).unwrap();
                    v
                }),
            );
        )*
    };
}
