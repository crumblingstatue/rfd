use crate::DialogOptions;
use std::path::PathBuf;

mod utils {
    use gtk_sys::{GtkFileChooser, GtkResponseType};

    use std::ptr;
    use std::{ffi::c_void, path::PathBuf};
    use std::{ffi::CStr, os::raw::c_char};

    use gobject_sys::GCallback;

    #[repr(i32)]
    pub enum GtkFileChooserAction {
        Open = 0,
        Save = 1,
        SelectFolder = 2,
        // CreateFolder = 3,
    }

    pub unsafe fn build_gtk_dialog(
        title: &str,
        action: GtkFileChooserAction,
        btn1: &str,
        btn2: &str,
    ) -> *mut GtkFileChooser {
        let dialog = gtk_sys::gtk_file_chooser_dialog_new(
            title.as_ptr() as *const _,
            ptr::null_mut(),
            action as i32,
            btn1.as_ptr() as *const _,
            gtk_sys::GTK_RESPONSE_CANCEL,
            btn2.as_ptr() as *const _,
            gtk_sys::GTK_RESPONSE_ACCEPT,
            ptr::null_mut::<i8>(),
        );
        dialog as _
    }

    pub unsafe fn add_filters(dialog: *mut GtkFileChooser, filters: &[crate::Filter]) {
        for f in filters.iter() {
            let filter = gtk_sys::gtk_file_filter_new();

            let name = format!("{}\0", f.name);
            let paterns: Vec<_> = f.extensions.iter().map(|e| format!("*.{}\0", e)).collect();

            gtk_sys::gtk_file_filter_set_name(filter, name.as_ptr() as *const _);

            for p in paterns.iter() {
                gtk_sys::gtk_file_filter_add_pattern(filter, p.as_ptr() as *const _);
            }

            gtk_sys::gtk_file_chooser_add_filter(dialog, filter);
        }
    }

    /// gtk_init_check()
    pub unsafe fn init_check() -> bool {
        gtk_sys::gtk_init_check(ptr::null_mut(), ptr::null_mut()) == 1
    }

    pub unsafe fn wait_for_cleanup() {
        while gtk_sys::gtk_events_pending() == 1 {
            gtk_sys::gtk_main_iteration();
        }
    }

    //
    // Getting paths from dialog
    //

    pub unsafe fn get_result(dialog: *mut GtkFileChooser) -> Option<PathBuf> {
        let chosen_filename = gtk_sys::gtk_file_chooser_get_filename(dialog as *mut _);

        let cstr = CStr::from_ptr(chosen_filename).to_str();

        if let Ok(cstr) = cstr {
            Some(PathBuf::from(cstr.to_owned()))
        } else {
            None
        }
    }
    pub unsafe fn get_results(dialog: *mut GtkFileChooser) -> Vec<PathBuf> {
        #[derive(Debug)]
        struct FileList(*mut glib_sys::GSList);

        impl Iterator for FileList {
            type Item = glib_sys::GSList;
            fn next(&mut self) -> Option<Self::Item> {
                let curr_ptr = self.0;

                if !curr_ptr.is_null() {
                    let curr = unsafe { *curr_ptr };

                    self.0 = curr.next;

                    Some(curr)
                } else {
                    None
                }
            }
        }

        let chosen_filenames = gtk_sys::gtk_file_chooser_get_filenames(dialog as *mut _);

        let paths: Vec<PathBuf> = FileList(chosen_filenames)
            .filter_map(|item| {
                let cstr = CStr::from_ptr(item.data as _).to_str();

                if let Ok(cstr) = cstr {
                    Some(PathBuf::from(cstr.to_owned()))
                } else {
                    None
                }
            })
            .collect();

        paths
    }

    //
    // ASYNC
    //

    unsafe fn connect_raw<F>(
        receiver: *mut gobject_sys::GObject,
        signal_name: *const c_char,
        trampoline: GCallback,
        closure: *mut F,
    ) {
        use std::mem;

        use glib_sys::gpointer;

        unsafe extern "C" fn destroy_closure<F>(ptr: *mut c_void, _: *mut gobject_sys::GClosure) {
            // destroy
            Box::<F>::from_raw(ptr as *mut _);
        }
        assert_eq!(mem::size_of::<*mut F>(), mem::size_of::<gpointer>());
        assert!(trampoline.is_some());
        let handle = gobject_sys::g_signal_connect_data(
            receiver,
            signal_name,
            trampoline,
            closure as *mut _,
            Some(destroy_closure::<F>),
            0,
        );
        assert!(handle > 0);
        // from_glib(handle)
    }

    pub unsafe fn connect_response<F: Fn(GtkResponseType) + 'static>(
        dialog: *mut GtkFileChooser,
        f: F,
    ) {
        use std::mem::transmute;

        unsafe extern "C" fn response_trampoline<F: Fn(GtkResponseType) + 'static>(
            this: *mut gtk_sys::GtkDialog,
            res: GtkResponseType,
            f: glib_sys::gpointer,
        ) {
            let f: &F = &*(f as *const F);

            f(res);
            // f(
            //     &Dialog::from_glib_borrow(this).unsafe_cast_ref(),
            //     from_glib(response_id),
            // )
        }
        let f: Box<F> = Box::new(f);
        connect_raw(
            dialog as *mut _,
            b"response\0".as_ptr() as *const _,
            Some(transmute::<_, unsafe extern "C" fn()>(
                response_trampoline::<F> as *const (),
            )),
            Box::into_raw(f),
        );
    }
}

use utils::*;

pub fn pick_file<'a>(params: impl Into<Option<DialogOptions<'a>>>) -> Option<PathBuf> {
    let params = params.into().unwrap_or_default();

    unsafe {
        let gtk_inited = init_check();

        if gtk_inited {
            let dialog = build_gtk_dialog(
                "Open File\0",
                GtkFileChooserAction::Open,
                "Cancel\0",
                "Open\0",
            );

            add_filters(dialog, &params.filters);

            let res = gtk_sys::gtk_dialog_run(dialog as *mut _);

            let out = if res == gtk_sys::GTK_RESPONSE_ACCEPT {
                get_result(dialog)
            } else {
                None
            };

            wait_for_cleanup();
            gtk_sys::gtk_widget_destroy(dialog as *mut _);
            wait_for_cleanup();

            out
        } else {
            None
        }
    }
}

pub fn save_file<'a>(params: impl Into<Option<DialogOptions<'a>>>) -> Option<PathBuf> {
    let params = params.into().unwrap_or_default();

    unsafe {
        let gtk_inited = init_check();

        if gtk_inited {
            let dialog = build_gtk_dialog(
                "Save File\0",
                GtkFileChooserAction::Save,
                "Cancel\0",
                "Save\0",
            );

            gtk_sys::gtk_file_chooser_set_do_overwrite_confirmation(dialog, 1);

            add_filters(dialog, &params.filters);

            let res = gtk_sys::gtk_dialog_run(dialog as *mut _);

            let out = if res == gtk_sys::GTK_RESPONSE_ACCEPT {
                get_result(dialog)
            } else {
                None
            };

            wait_for_cleanup();
            gtk_sys::gtk_widget_destroy(dialog as *mut _);
            wait_for_cleanup();

            out
        } else {
            None
        }
    }
}

pub fn pick_folder<'a>(params: impl Into<Option<DialogOptions<'a>>>) -> Option<PathBuf> {
    let params = params.into().unwrap_or_default();

    unsafe {
        let gtk_inited = init_check();

        if gtk_inited {
            let dialog = build_gtk_dialog(
                "Select Folder\0",
                GtkFileChooserAction::SelectFolder,
                "Cancel\0",
                "Select\0",
            );

            let res = gtk_sys::gtk_dialog_run(dialog as *mut _);

            let out = if res == gtk_sys::GTK_RESPONSE_ACCEPT {
                get_result(dialog)
            } else {
                None
            };

            wait_for_cleanup();
            gtk_sys::gtk_widget_destroy(dialog as *mut _);
            wait_for_cleanup();

            out
        } else {
            None
        }
    }
}

pub fn pick_files<'a>(params: impl Into<Option<DialogOptions<'a>>>) -> Option<Vec<PathBuf>> {
    let params = params.into().unwrap_or_default();

    #[derive(Debug)]
    struct FileList(*mut glib_sys::GSList);

    impl Iterator for FileList {
        type Item = glib_sys::GSList;
        fn next(&mut self) -> Option<Self::Item> {
            let curr_ptr = self.0;

            if !curr_ptr.is_null() {
                let curr = unsafe { *curr_ptr };

                self.0 = curr.next;

                Some(curr)
            } else {
                None
            }
        }
    }

    unsafe {
        let gtk_inited = init_check();

        if gtk_inited {
            let dialog = build_gtk_dialog(
                "Open File\0",
                GtkFileChooserAction::Open,
                "Cancel\0",
                "Open\0",
            );

            gtk_sys::gtk_file_chooser_set_select_multiple(dialog, 1);

            add_filters(dialog, &params.filters);

            let res = gtk_sys::gtk_dialog_run(dialog as *mut _);

            let out = if res == gtk_sys::GTK_RESPONSE_ACCEPT {
                Some(get_results(dialog))
            } else {
                None
            };

            wait_for_cleanup();
            gtk_sys::gtk_widget_destroy(dialog as *mut _);
            wait_for_cleanup();

            out
        } else {
            None
        }
    }
}