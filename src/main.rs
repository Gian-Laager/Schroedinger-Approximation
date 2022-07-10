mod airy;
mod utils;
mod airy_wave_func;
mod newtons_method;

use crate::airy::airy_ai;
use crate::airy_wave_func::AiryWaveFunction;
use num::complex::Complex64;
use num::pow::Pow;
use rayon::iter::*;
use std::f64;
use std::fs::File;
use std::io::Write;
use tokio;

const TRAPEZE_PER_THREAD: usize = 1000;
const INTEG_STEPS: usize = 10000;
const NUMBER_OF_POINTS: usize = 10000;
const H_BAR: f64 = 1.0;
const X_0: f64 = 8.0;
const ENERGY: f64 = 20.0;
const MASS: f64 = 3.0;
const C_0: f64 = 1.0;
const THETA: f64 = 0.0;

const VIEW: (f64, f64) = (-8.0, 8.0);

trait ReToC: Sync {
    fn eval(&self, x: &f64) -> Complex64;
}

struct Function {
    f: fn(f64) -> Complex64,
}

impl Function {
    const fn new(f: fn(f64) -> Complex64) -> Function {
        return Function { f };
    }
}

impl ReToC for Function {
    fn eval(&self, x: &f64) -> Complex64 {
        (self.f)(*x)
    }
}

#[derive(Copy, Clone)]
pub struct Phase {
    energy: f64,
    mass: f64,
    potential: fn(&f64) -> f64,
    x_0: f64,
}

impl Phase {
    const fn new(energy: f64, mass: f64, potential: fn(&f64) -> f64, x_0: f64) -> Phase {
        return Phase {
            energy,
            mass,
            potential,
            x_0
        };
    }
}

impl ReToC for Phase {
    fn eval(&self, x: &f64) -> Complex64 {
        return (complex(2.0, 0.0)
            * complex(self.mass, 0.0)
            * complex((self.potential)(x) - self.energy, 0.0))
            .sqrt()
            / complex(H_BAR, 0.0);
    }
}

fn complex(re: f64, im: f64) -> Complex64 {
    return Complex64 { re, im };
}

fn trapezoidal_approx(start: &Point, end: &Point) -> Complex64 {
    return complex(end.x - start.x, 0.0) * (start.y + end.y) / complex(2.0, 0.0);
}

fn index_to_range(x: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    return (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
}

struct Point {
    x: f64,
    y: Complex64,
}

fn integrate(points: Vec<Point>, batch_size: usize) -> Complex64 {
    if points.len() < 2 {
        return complex(0.0, 0.0);
    }

    let batches: Vec<&[Point]> = points.chunks(batch_size).collect();

    let parallel: Complex64 = batches
        .par_iter()
        .map(|batch| {
            let mut sum = complex(0.0, 0.0);
            for i in 0..(batch.len() - 1) {
                sum += trapezoidal_approx(&batch[i], &batch[i + 1]);
            }
            return sum;
        })
        .sum();

    let mut rest = complex(0.0, 0.0);

    for i in 0..batches.len() - 1 {
        rest += trapezoidal_approx(&batches[i][batches[i].len() - 1], &batches[i + 1][0]);
    }

    return parallel + rest;
}

fn evaluate_function_between(f: &dyn ReToC, a: f64, b: f64, n: usize) -> Vec<Point> {
    if a == b {
        return vec![];
    }

    (0..n)
        .into_par_iter()
        .map(|i| index_to_range(i as f64, 0.0, n as f64 - 1.0, a, b))
        .map(|x| Point { x, y: f.eval(&x) })
        .collect()
}

pub struct WkbWaveFunction<'a> {
    c_plus: f64,
    c_minus: f64,
    phase: &'a Phase,
    integration_steps: usize,
}

impl WkbWaveFunction<'_> {
    fn new(phase: &Phase, c0: f64, theta: f64, integration_steps: usize) -> WkbWaveFunction {
        let c_plus = 0.5 * c0 * f64::cos(theta - std::f64::consts::PI / 4.0);
        let c_minus = -0.5 * c0 * f64::sin(theta - std::f64::consts::PI / 4.0);
        return WkbWaveFunction {
            c_plus,
            c_minus,
            phase,
            integration_steps,
        };
    }
}

impl ReToC for WkbWaveFunction<'_> {
    fn eval(&self, x: &f64) -> Complex64 {
        let integral = integrate(
            evaluate_function_between(self.phase, self.phase.x_0, *x, self.integration_steps),
            TRAPEZE_PER_THREAD,
        );

        return (complex(self.c_plus, 0.0) * integral.exp()
            + complex(self.c_minus, 0.0) * (-integral).exp())
            / (self.phase.eval(&x)).sqrt();
    }
}

fn square(x: &f64) -> f64 {
    // 5.0 * (x + 1.0) * (x - 1.0) * (x + 2.0) * (x - 2.0) - 1.0
    x * x
}

fn order_ts((t1, t2): &(f64, f64)) -> (f64, f64) {
    return if t1 > t2 { (*t2, *t1) } else { (*t1, *t2) };
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let phase1: Phase = Phase::new(ENERGY, MASS, square, -X_0);
    let wave_func1 = WkbWaveFunction::new(&phase1, C_0, THETA, INTEG_STEPS);
    let values1 = evaluate_function_between(&wave_func1, VIEW.0, 0.0, NUMBER_OF_POINTS);
    let phase2: Phase = Phase::new(ENERGY, MASS, square, X_0);
    let wave_func2 = WkbWaveFunction::new(&phase2, C_0, THETA, INTEG_STEPS);
    let values2 = evaluate_function_between(&wave_func2, 0.0, VIEW.1, NUMBER_OF_POINTS);
    let turning_point_boundaries = &AiryWaveFunction::calc_ts(&phase1, VIEW).ts;

    let mut data_file = File::create("data.txt").unwrap();

    let mut data_str: String = values1
        .par_iter()
        .filter(|p| {
            (turning_point_boundaries.clone()).map(|ts| ts.iter()
                .map(order_ts)
                .map(|(t1, t2)| p.x < t1 || p.x > t2)
                .fold(true, |a, b| a && b))
                .unwrap_or(true)
        })
        .map(|p| -> String { format!("{} {} {}\n", p.x, p.y.re, p.y.im) })
        .reduce(|| String::new(), |s: String, current: String| s + &*current);

    data_str.push_str(&*values2
        .par_iter()
        .filter(|p| {
            (turning_point_boundaries.clone()).map(|ts| ts.iter()
                .map(order_ts)
                .map(|(t1, t2)| p.x < t1 || p.x > t2)
                .fold(true, |a, b| a && b))
                .unwrap_or(true)
        })
        .map(|p| -> String { format!("{} {} {}\n", p.x, p.y.re, p.y.im) })
        .reduce(|| String::new(), |s: String, current: String| s + &*current));

    let airy_wave_funcs = AiryWaveFunction::new(&wave_func1, (VIEW.0, 0.0));

    let airy_data_str = airy_wave_funcs.iter().map(|airy_wave_func| {
        let airy_values = evaluate_function_between(airy_wave_func, airy_wave_func.ts.0, airy_wave_func.ts.1, NUMBER_OF_POINTS);

        airy_values
            .par_iter()
            .map(|p| -> String { format!("{} {} {}\n", p.x, p.y.re, p.y.im) })
            .reduce(|| String::new(), |s: String, current: String| s + &*current)
    })
        .fold(String::new(), |s: String, current: String| s + "\n\n" + &*current);
    data_str.push_str(&*airy_data_str);

    let airy_wave_funcs = AiryWaveFunction::new(&wave_func2, (0.0, VIEW.1));

    let airy_data_str = airy_wave_funcs.iter().map(|airy_wave_func| {
        let airy_values = evaluate_function_between(airy_wave_func, airy_wave_func.ts.0, airy_wave_func.ts.1, NUMBER_OF_POINTS);

        airy_values
            .par_iter()
            .map(|p| -> String { format!("{} {} {}\n", p.x, p.y.re, p.y.im) })
            .reduce(|| String::new(), |s: String, current: String| s + &*current)
    })
        .fold(String::new(), |s: String, current: String| s + "\n\n" + &*current);
    data_str.push_str(&*airy_data_str);

    data_file
        .write_all((data_str).as_ref()).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    fn square(x: f64) -> Complex64 {
        return complex(x * x, 0.0);
    }

    fn square_itegral(a: f64, b: f64) -> Complex64 {
        return complex(b * b * b / 3.0 - a * a * a / 3.0, 0.0);
    }

    fn float_compare(expect: Complex64, actual: Complex64, epsilon: f64) -> bool {
        let average = (expect.norm() + actual.norm()) / 2.0;
        return (expect - actual).norm() / average < epsilon;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn integral_of_square() {
        static SQUARE_FUNC: Function = Function::new(square);
        for i in 0..100 {
            for j in 0..10 {
                let a = f64::from(i - 50) / 12.3;
                let b = f64::from(j - 50) / 12.3;

                if i == j {
                    assert_eq!(
                        integrate(
                            evaluate_function_between(&SQUARE_FUNC, a, b, INTEG_STEPS),
                            TRAPEZE_PER_THREAD,
                        ),
                        complex(0.0, 0.0)
                    );
                    continue;
                }

                let epsilon = 0.00001;
                assert!(float_compare(
                    integrate(
                        evaluate_function_between(&SQUARE_FUNC, a, b, INTEG_STEPS),
                        TRAPEZE_PER_THREAD,
                    ),
                    square_itegral(a, b),
                    epsilon,
                ));
            }
        }
    }

    #[test]
    fn evaluate_square_func_between() {
        static SQUARE_FUNC: Function = Function::new(square);
        let actual = evaluate_function_between(&SQUARE_FUNC, -2.0, 2.0, 5);
        let expected = vec![
            Point {
                x: -2.0,
                y: complex(4.0, 0.0),
            },
            Point {
                x: -1.0,
                y: complex(1.0, 0.0),
            },
            Point {
                x: 0.0,
                y: complex(0.0, 0.0),
            },
            Point {
                x: 1.0,
                y: complex(1.0, 0.0),
            },
            Point {
                x: 2.0,
                y: complex(4.0, 0.0),
            },
        ];

        for (a, e) in actual.iter().zip(expected) {
            assert_eq!(a.x, e.x);
            assert_eq!(a.y, e.y);
        }
    }

    fn sinusoidal_exp_complex(x: f64) -> Complex64 {
        return complex(x, x).exp();
    }

    fn sinusoidal_exp_complex_integral(a: f64, b: f64) -> Complex64 {
        // (-1/2 + i/2) (e^((1 + i) a) - e^((1 + i) b))
        return complex(-0.5, 0.5) * (complex(a, a).exp() - complex(b, b).exp());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn integral_of_sinusoidal_exp() {
        static SINUSOIDAL_EXP_COMPLEX: Function = Function::new(sinusoidal_exp_complex);
        for i in 0..10 {
            for j in 0..10 {
                let a = f64::from(i - 50) / 12.3;
                let b = f64::from(j - 50) / 12.3;

                if i == j {
                    assert_eq!(
                        integrate(
                            evaluate_function_between(&SINUSOIDAL_EXP_COMPLEX, a, b, INTEG_STEPS),
                            TRAPEZE_PER_THREAD,
                        ),
                        complex(0.0, 0.0)
                    );
                    continue;
                }
                let epsilon = 0.0001;
                assert!(float_compare(
                    integrate(
                        evaluate_function_between(&SINUSOIDAL_EXP_COMPLEX, a, b, INTEG_STEPS),
                        TRAPEZE_PER_THREAD,
                    ),
                    sinusoidal_exp_complex_integral(a, b),
                    epsilon,
                ));
            }
        }
    }
}
