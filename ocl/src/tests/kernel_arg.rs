use std::thread;
use std::time::Duration;
use standard::{ProQue, Kernel, Buffer};

static SRC: &'static str = r#"
    __kernel void add(__global float* buffer, float addend) {
        buffer[get_global_id(0)] += addend;
    }
"#;


fn set_arg(kernel: &mut Kernel, pro_que: &ProQue) {
    // let throwaway_vec = vec![99usize; 1 << 22];
    let buffer = pro_que.create_buffer::<f32>().unwrap();
    // assert!(throwaway_vec[1 << 15] == 99);

    kernel.set_arg("buf", Some(&buffer)).unwrap();
}


/// Create a vector on the heap, then a buffer right after it. Assign the
/// buffer as a kernel argument. Let them both fall out of scope. Run kernel.
/// If a copy of the pointer to the buffer is not made by the kernel, it will
/// be deallocated and when the kernel tries to access it via the pointer it
/// has internally stored within its argument, it may cause a segfault or
/// error.
///
// FIXME: Actually check for a meaningful value.
#[test]
fn kernel_arg_ptr_out_of_scope() {
    let pro_que = ProQue::builder()
        .src(SRC)
        .dims([1024])
        .build().unwrap();

    let mut kernel = pro_que.kernel_builder("add")
        .arg_named("buf", None::<&Buffer<f32>>)
        .arg(10.0f32)
        .build().unwrap();

    set_arg(&mut kernel, &pro_que);
    // let throwaway_vec = vec![99usize; 1 << 24];
    thread::sleep(Duration::from_millis(100));

    for _ in 0..5 {
        unsafe { kernel.enq().unwrap(); }
    }


    // assert!(throwaway_vec[1 << 15] == 99);
}


/// Ensure that owned buffer/image kernel arguments work and that they do not
/// unnecessarily restrict the lifetime of `KernelBuilder`.
#[test]
fn kernel_arg_owned_mem() {
    let ds_len = 1024;
    let pro_que = ProQue::builder()
        .src(SRC)
        .dims(ds_len)
        .build().unwrap();

    let buffer = pro_que.create_buffer::<f32>().unwrap();

    let kernel_builder = {
        let buf_clone = buffer.clone();

        // Owned buffer is passed.
        let mut kb = pro_que.kernel_builder("add");
        kb.arg_named("buf", buf_clone);
        kb.arg(10.0f32);
        kb
    };

    let kernel = kernel_builder.build().unwrap();

    thread::sleep(Duration::from_millis(100));

    for _ in 0..5 {
        unsafe { kernel.enq().unwrap(); }
    }

    let mut output_vec = vec![1000.; ds_len * 2];
    buffer.read(&mut output_vec).len(ds_len).enq().unwrap();

    for (idx, e) in output_vec.iter().enumerate() {
        if idx < ds_len {
            assert_eq!(*e, 50.);
        } else {
            assert_eq!(*e, 1000.);
        }
    }
}