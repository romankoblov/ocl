extern crate libc;
extern crate ocl;

use libc::c_void;
use ocl::{Context, ProQue, ProgramBuilder, SimpleDims, Buffer, EventList};
use ocl::cl_h::{cl_event, cl_int};

// How many iterations we wish to run:
const ITERATIONS: usize = 8;
// Whether or not to print:
const PRINT_DEBUG: bool = true;
// How many results to print from each iteration:
const RESULTS_TO_PRINT: usize = 5;

struct TestEventsStuff {
    seed_env: *const Buffer<u32>, 
    res_env: *const Buffer<u32>, 
    data_set_size: usize,
    addend: u32, 
    itr: usize,
}

// Callback for `test_events()`.
extern fn _test_events_verify_result(event: cl_event, status: cl_int, user_data: *mut c_void) {
    let buncha_stuff = user_data as *const TestEventsStuff;

    

    unsafe {
        let seed_buffer: *const Buffer<u32> = (*buncha_stuff).seed_env as *const Buffer<u32>;
        let result_buffer: *const Buffer<u32> = (*buncha_stuff).res_env as *const Buffer<u32>;
        let data_set_size: usize = (*buncha_stuff).data_set_size;
        let addend: u32 = (*buncha_stuff).addend;
        let itr: usize = (*buncha_stuff).itr;
        
        if PRINT_DEBUG { println!("\nEvent: `{:?}` has completed with status: `{}`, data_set_size: '{}`, \
                 addend: {}, itr: `{}`.", event, status, data_set_size, addend, itr); }

        for idx in 0..data_set_size {
            assert_eq!((*result_buffer)[idx], 
                ((*seed_buffer)[idx] + ((itr + 1) as u32) * addend));

            if PRINT_DEBUG && (idx < RESULTS_TO_PRINT) {
                let correct_result = (*seed_buffer)[idx] + (((itr + 1) as u32) * addend);
                print!("correct_result: {}, result_buffer[{idx}]:{}\n",
                    correct_result, (*result_buffer)[idx], idx = idx);
            }
        }

        let mut errors_found = 0;

        for idx in 0..data_set_size {
            // [FIXME]: FAILING ON OSX -- TEMPORARLY COMMENTING OUT
            // assert_eq!((*result_buffer)[idx], 
            //  ((*seed_buffer)[idx] + ((itr + 1) as u32) * addend));

            if PRINT_DEBUG {
                let correct_result = (*seed_buffer)[idx] + (((itr + 1) as u32) * addend);

                if (*result_buffer)[idx] != correct_result {
                    print!("correct_result:{}, result_buffer[{idx}]:{}\n",
                        correct_result, (*result_buffer)[idx], idx = idx);

                    errors_found += 1;
                }
            }
        }

        if PRINT_DEBUG { 
            if errors_found > 0 { print!("TOTAL ERRORS FOUND: {}\n", errors_found); }
        }
    }
}

fn main() {
    // Create a context & program/queue: 
    let mut ocl_pq = ProQue::new(&Context::new_by_index_and_type(None, None).unwrap(), None);

    // Build program:
    ocl_pq.build_program(ProgramBuilder::new().src_file("cl/kernel_file.cl")).unwrap();

    // Set up data set size and work dimensions:
    let data_set_size = 900000;
    let our_test_dims = SimpleDims::One(data_set_size);

    // Create source and result buffers (our data containers):
    let seed_buffer = Buffer::with_vec_scrambled((0u32, 500u32), &our_test_dims, &ocl_pq.queue());
    let mut result_buffer = Buffer::with_vec(&our_test_dims, &ocl_pq.queue());

    // Our arbitrary addend:
    let addend = 11u32;

    // Create kernel with the source initially set to our seed values.
    let mut kernel = ocl_pq.create_kernel_with_dims("add_scalar", our_test_dims.clone())
        .arg_buf_named("src", Some(&seed_buffer))
        .arg_scl(addend)
        .arg_buf(&mut result_buffer);

    // Create event list:
    let mut kernel_event = EventList::new();    

    //#############################################################################################

    // Create storage for per-event data:
    let mut buncha_stuffs = Vec::<TestEventsStuff>::with_capacity(ITERATIONS);

    // Run our test:
    for itr in 0..ITERATIONS {
        // Store information for use by the result callback function into a vector
        // which will persist until all of the commands have completed (as long as
        // we are sure to allow the queue to finish before returning).
        buncha_stuffs.push(TestEventsStuff {
            seed_env: &seed_buffer as *const Buffer<u32>,
            res_env: &result_buffer as *const Buffer<u32>, 
            data_set_size: data_set_size, 
            addend: addend, 
            itr: itr,
        });

        // Change the source buffer to the result after seed values have been copied.
        // Yes, this is far from optimal...
        // Should just copy the values in the first place but oh well.
        if itr != 0 {
            kernel.set_arg_buf_named("src", Some(&result_buffer)).unwrap();
        }

        if PRINT_DEBUG { println!("Enqueuing kernel [itr:{}]...", itr); }
        kernel.enqueue_with_events(None, Some(&mut kernel_event));

        let mut read_event = EventList::new();
        
        if PRINT_DEBUG { println!("Enqueuing read buffer [itr:{}]...", itr); }
        unsafe { result_buffer.fill_vec_async(None, Some(&mut read_event)).unwrap(); }
    
        // Clone event list just for fun:
        let read_event = read_event.clone();

        let last_idx = buncha_stuffs.len() - 1;     

        unsafe {
            if PRINT_DEBUG { println!("Setting callback (verify_result, buncha_stuff[{}]) [i:{}]...", 
                last_idx, itr); }
            read_event.set_callback(_test_events_verify_result, 
                // &mut buncha_stuffs[last_idx] as *mut _ as *mut c_void);
                &mut buncha_stuffs[last_idx]).unwrap();
        }

        // if PRINT_DEBUG { println!("Releasing read_event [i:{}]...", itr); }
        // // Decrement reference count. Will still complete before releasing.
        // read_event.release_all();
    }

    // Wait for all queued tasks to finish so that verify_result() will be called:
    ocl_pq.queue().finish();
}

