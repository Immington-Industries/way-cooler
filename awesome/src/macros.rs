//! Warning: extreme macros

/// Create a keypress using fewer keystrokes. Provides a custom panic method.
#[macro_export]
macro_rules! keypress {
    ($modifier:expr, $key:expr) => {
        $crate::keys::KeyPress::from_key_names(&[$modifier], $key).expect(concat!(
            "Unable to create keypress from macro with ",
            $modifier,
            " and ",
            $key
        ))
    };
}

macro_rules! impl_objectable {
    ($WrapperType:ident, $StateType:ty) => {
        impl<'lua> Objectable<'lua, $WrapperType<'lua>, $StateType> for $WrapperType<'lua> {
            fn _wrap(object: Object<'lua>) -> $WrapperType {
                $WrapperType(object)
            }

            fn state(&self) -> $crate::rlua::Result<::std::cell::Ref<$StateType>> {
                Ok(self.0.object.borrow::<$StateType>()?)
            }

            fn get_object_mut(&mut self) -> $crate::rlua::Result<::std::cell::RefMut<$StateType>> {
                Ok(self.0.object.borrow_mut::<$StateType>()?)
            }
        }
    };
}
