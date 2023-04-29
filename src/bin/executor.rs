#![warn(clippy::pedantic)]

extern crate fiona;

pub fn main() {
    let mut ioc = fiona::IoContext::new();

    for idx in 0..5 {
        let ex = ioc.get_executor();
        ioc.post(Box::pin(async move {
            println!("Starting the timer coro...");

            let mut timer = fiona::time::Timer::new(&ex);
            let time = (idx + 1) * 1000;
            timer.expires_after(std::time::Duration::from_millis(time));

            println!("Suspending now...");
            match timer.async_wait().await {
                Ok(_) => {
                    println!("waited successfully for {} seconds!", idx + 1);
                }
                Err(_) => {
                    println!("Timer read failed!");
                }
            }

            println!("Going to wait again...");
            match timer.async_wait().await {
                Ok(_) => {
                    println!("waited succesfully, again, for {} seconds", idx + 1);
                }
                Err(_) => {
                    println!("Timer read failed!");
                }
            }
        }));
    }

    ioc.run();

    println!("All tasks completed running");
}
