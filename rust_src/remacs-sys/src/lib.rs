#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(improper_ctypes)]
#![cfg_attr(feature = "strict", deny(warnings))]
#![feature(never_type)]
#![feature(specialization)]

extern crate field_offset;

use libc::timespec;

pub use crate::data::*;

pub mod time;
pub use crate::time::LispTime;

pub mod lisp;
pub use crate::lisp::LispObject;

// Some names conflict with the generated ones
pub mod data {
    use field_offset::FieldOffset;

    use crate::{EmacsInt, LispObject};

    /// These are the types of forwarding objects used in the value slot
    /// of symbols for special built-in variables whose value is stored in
    /// C/Rust static variables.
    pub type Lisp_Fwd_Type = u32;
    pub const Lisp_Fwd_Int: Lisp_Fwd_Type = 0; // Fwd to a C `int' variable.
    pub const Lisp_Fwd_Bool: Lisp_Fwd_Type = 1; // Fwd to a C boolean var.
    pub const Lisp_Fwd_Obj: Lisp_Fwd_Type = 2; // Fwd to a C LispObject variable.
    pub const Lisp_Fwd_Buffer_Obj: Lisp_Fwd_Type = 3; // Fwd to a LispObject field of buffers.
    pub const Lisp_Fwd_Kboard_Obj: Lisp_Fwd_Type = 4; // Fwd to a LispObject field of kboards.

    // these structs will still need to be compatible with their C
    // counterparts until all the C callers of the DEFVAR macros are
    // ported to Rust. However, as do_symval_forwarding and
    // store_symval_forwarding have been ported, some Rust-isms have
    // started to happen.

    #[repr(C)]
    pub union Lisp_Fwd {
        pub u_intfwd: Lisp_Intfwd,
        pub u_boolfwd: Lisp_Boolfwd,
        pub u_objfwd: Lisp_Objfwd,
        pub u_buffer_objfwd: Lisp_Buffer_Objfwd,
        pub u_kboard_objfwd: Lisp_Kboard_Objfwd,
    }

    /// Forwarding pointer to an int variable.
    /// This is allowed only in the value cell of a symbol,
    /// and it means that the symbol's value really lives in the
    /// specified int variable.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Lisp_Intfwd {
        pub ty: Lisp_Fwd_Type, // = Lisp_Fwd_Int
        pub intvar: *mut EmacsInt,
    }

    /// Boolean forwarding pointer to an int variable.
    /// This is like Lisp_Intfwd except that the ostensible
    /// "value" of the symbol is t if the bool variable is true,
    /// nil if it is false.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Lisp_Boolfwd {
        pub ty: Lisp_Fwd_Type, // = Lisp_Fwd_Bool
        pub boolvar: *mut bool,
    }

    /// Forwarding pointer to a LispObject variable.
    /// This is allowed only in the value cell of a symbol,
    /// and it means that the symbol's value really lives in the
    /// specified variable.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Lisp_Objfwd {
        pub ty: Lisp_Fwd_Type, // = Lisp_Fwd_Obj
        pub objvar: *mut LispObject,
    }

    /// Like Lisp_Objfwd except that value lives in a slot in the
    /// current buffer.  Value is byte index of slot within buffer.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Lisp_Buffer_Objfwd {
        pub ty: Lisp_Fwd_Type, // = Lisp_Fwd_Buffer_Obj
        pub offset: FieldOffset<crate::Lisp_Buffer, LispObject>,
        // One of Qnil, Qintegerp, Qsymbolp, Qstringp, Qfloatp or Qnumberp.
        pub predicate: LispObject,
    }

    /// Like Lisp_Objfwd except that value lives in a slot in the
    /// current kboard.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Lisp_Kboard_Objfwd {
        pub ty: Lisp_Fwd_Type, // = Lisp_Fwd_Kboard_Obj
        pub offset: FieldOffset<crate::kboard, LispObject>,
    }
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
include!(concat!(env!("OUT_DIR"), "/definitions.rs"));

pub type Lisp_Object = LispObject;

include!(concat!(env!("OUT_DIR"), "/globals.rs"));

pub type Lisp_Buffer = buffer;

// In order to use `lazy_static!` with LispSubr, it must be Sync. Raw
// pointers are not Sync, but it isn't a problem to define Sync if we
// never mutate LispSubr values. If we do, we will need to create
// these objects at runtime, perhaps using forget().
//
// Based on http://stackoverflow.com/a/28116557/509706
unsafe impl Sync for Lisp_Subr {}
