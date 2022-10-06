#![warn(clippy::pedantic)]

extern crate rio;

pub fn main() {
  let mut ioc = rio::IoContext::new();

  for idx in 0..5 {
    let io = ioc.clone();
    ioc.post(Box::new(async move {
      println!("Starting the timer coro...");

      let mut timer = rio::io::Timer::new(io);
      let time = (idx + 1) * 1000;
      timer.expires_after(time);

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
