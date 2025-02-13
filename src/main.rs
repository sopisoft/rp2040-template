#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(core_intrinsics)]

use core::{cell::RefCell, f32::consts::PI};

use defmt::info;
use defmt_rtt as _;

use fugit::{MicrosDuration, MicrosDurationU32};
use libm::cosf;

use cortex_m::interrupt::{free, Mutex};
use embedded_hal::pwm::SetDutyCycle;
use rp_pico::hal::{
    self, pac,
    pac::interrupt,
    pwm,
    timer::{self, Alarm},
};

#[cfg(debug_assertions)]
extern crate panic_probe;

#[cfg(not(debug_assertions))]
#[inline(never)]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::intrinsics::abort()
}

type LedScheduler = (
    pwm::Channel<pwm::Slice<pwm::Pwm4, pwm::FreeRunning>, pwm::B>, // Channel
    timer::Alarm0,                                                 // Alarm
    MicrosDuration<u32>,                                           // Interval
    f32,                                                           // Phase
);
static LED_SCHEDULER: Mutex<RefCell<Option<LedScheduler>>> = Mutex::new(RefCell::new(None));

#[hal::entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .unwrap();

    let mut timer = hal::timer::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let slice = pwm::Slices::new(pac.PWM, &mut pac.RESETS);
    let mut pwm = slice.pwm4;
    pwm.set_ph_correct();
    pwm.enable();

    let mut channel = pwm.channel_b;
    channel.output_to(pins.gpio25);

    free(|cs| {
        let interval = MicrosDurationU32::millis(25);
        let mut alarm0 = timer.alarm_0().unwrap();
        let _ = alarm0.schedule(interval);
        let _ = alarm0.enable_interrupt();
        let phase = 0.0;
        LED_SCHEDULER
            .borrow(cs)
            .replace(Some((channel, alarm0, interval, phase)));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    };

    loop {}
}

#[interrupt]
fn TIMER_IRQ_0() {
    free(|cs| {
        let scheduler = LED_SCHEDULER.borrow(cs).take();
        if let Some((mut channel, mut alarm0, interval, mut phase)) = scheduler {
            alarm0.clear_interrupt();

            let duty_percent = (cosf(phase * PI) * 50.0 + 50.0) as u8;
            let _ = channel.set_duty_cycle_percent(duty_percent);
            info!("Duty: {}%", duty_percent);

            let _ = phase += 0.025;
            let _ = alarm0.schedule(interval);

            LED_SCHEDULER
                .borrow(cs)
                .replace(Some((channel, alarm0, interval, phase)));
        }
    });
}
