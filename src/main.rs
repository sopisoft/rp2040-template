#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(core_intrinsics)]

use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

use defmt::info;
use defmt_rtt as _;

use fugit::{MicrosDuration, MicrosDurationU32};

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
    pwm::Channel<pwm::Slice<pwm::Pwm4, pwm::FreeRunning>, pwm::B>,
    timer::Alarm0,
    MicrosDuration<u32>,
);
type LedInfo = (u8, bool);
static LED_SCHEDULER: Mutex<RefCell<Option<LedScheduler>>> = Mutex::new(RefCell::new(None));
static LED_INFO: Mutex<RefCell<LedInfo>> = Mutex::new(RefCell::new((50, true)));

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
        LED_SCHEDULER
            .borrow(cs)
            .replace(Some((channel, alarm0, interval)));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    };

    loop {}
}

#[interrupt]
fn TIMER_IRQ_0() {
    free(|cs| {
        if let Some((channel, alarm0, interval)) = LED_SCHEDULER.borrow(cs).borrow_mut().deref_mut()
        {
            let _ = alarm0.clear_interrupt();

            let (mut duty_percent, mut is_fading_in) = LED_INFO.borrow(cs).borrow().deref();
            if is_fading_in {
                duty_percent += 1;
                if duty_percent == 100 {
                    is_fading_in = false;
                    info!("Fading out");
                }
            } else {
                duty_percent -= 1;
                if duty_percent == 0 {
                    is_fading_in = true;
                    info!("Fading in");
                }
            }
            let _ = duty_percent.min(100).max(0);

            let _ = channel.set_duty_cycle_percent(duty_percent);
            LED_INFO.borrow(cs).replace((duty_percent, is_fading_in));

            let _ = alarm0.schedule(*interval);
        }
    });
}
