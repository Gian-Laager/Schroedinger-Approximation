use num::signum;
use crate::*;

fn Ai(x: Complex64) -> Complex64 {
    let go_return;
    unsafe {
        go_return = airy_ai(x.re, x.im);
    }
    return complex(go_return.r0, go_return.r1);
}

fn Bi(x: Complex64) -> Complex64 {
    return -complex(0.0, 1.0) * Ai(x) + 2.0 * Ai(x * complex(-0.5, 3.0_f64.sqrt() / 2.0)) * complex(3_f64.sqrt() / 2.0, 0.5);
}

pub struct AiryWaveFunction {
    pub c_a: Complex64,
    pub c_b: Complex64,
    u_1: f64,
    x_1: f64,
    ts: Vec<(f64, f64)>,
}

impl AiryWaveFunction {
    fn get_u_1_cube_root(u_1: f64) -> Complex64 {
        complex(u_1, 0.0).pow(1.0 / 3.0) * Complex64::cis(2.0*f64::consts::TAU / 3.0)
    }

    fn calc_c_a_and_c_b(phase: &Phase, t: (f64, f64), c_wkb: (f64, f64), u_1: f64, x_1: f64) -> (Complex64, Complex64) {
        let u_1_cube_root = Self::get_u_1_cube_root(u_1);
        let wkb_plus_1 = integrate(evaluate_function_between(phase, X_0, t.0, INTEG_STEPS), TRAPEZE_PER_THREAD).exp() / phase.eval(&t.0).sqrt();
        let wkb_minus_1 = (-integrate(evaluate_function_between(phase, X_0, t.0, INTEG_STEPS), TRAPEZE_PER_THREAD)).exp() / phase.eval(&t.0).sqrt();
        let wkb_plus_2 = integrate(evaluate_function_between(phase, X_0, t.1, INTEG_STEPS), TRAPEZE_PER_THREAD).exp() / phase.eval(&t.1).sqrt();
        let wkb_minus_2 = (-integrate(evaluate_function_between(phase, X_0, t.1, INTEG_STEPS), TRAPEZE_PER_THREAD)).exp() / phase.eval(&t.1).sqrt();

        let airy_ai_1 = Ai(u_1_cube_root * (t.0 - x_1));
        let airy_bi_1 = Bi(u_1_cube_root * (t.0 - x_1));
        let airy_ai_2 = Ai(u_1_cube_root * (t.1 - x_1));
        let airy_bi_2 = Bi(u_1_cube_root * (t.1 - x_1));

        let c_a = ((-c_wkb.1 * (airy_bi_1 * wkb_minus_2 - airy_bi_2 * wkb_minus_1)) / (airy_ai_1 * airy_bi_2 - airy_ai_2 * airy_bi_1)) - ((c_wkb.0 * (airy_bi_1 * wkb_plus_2 - airy_bi_2 * wkb_plus_1)) / (airy_ai_1 * airy_bi_2 - airy_ai_2 * airy_bi_1));
        let c_b = ((c_wkb.1 * (airy_ai_1 * wkb_minus_2 - airy_ai_2 * wkb_minus_1)) / (airy_ai_1 * airy_bi_2 - airy_ai_2 * airy_bi_1)) + ((c_wkb.0 * (airy_ai_1 * wkb_plus_2 - airy_ai_2 * wkb_plus_1)) / (airy_ai_1 * airy_bi_2 - airy_ai_2 * airy_bi_1));

        return (c_a, c_b);
    }

    pub fn calc_ts(phase: &Phase) -> (f64, f64) {
        let t1 = signum(X_0) * f64::sqrt(phase.energy + H_BAR * H_BAR / phase.mass + f64::sqrt(H_BAR * H_BAR * (H_BAR * H_BAR + 2.0 * phase.mass * phase.energy)) / phase.mass);
        let t2 = signum(X_0) * f64::sqrt(phase.energy + H_BAR * H_BAR / phase.mass - f64::sqrt(H_BAR * H_BAR * (H_BAR * H_BAR + 2.0 * phase.mass * phase.energy)) / phase.mass);
        return (t1, t2);
    }

    pub fn new(wave_func: &WaveFunction) -> AiryWaveFunction {
        let phase = wave_func.phase;
        let (t1, t2) = AiryWaveFunction::calc_ts(phase);
        let x_1 = (t1 + t2) / 2.0;
        let u_1 = 2.0 * phase.mass / (H_BAR * H_BAR) * ((phase.potential)(&t1) - (phase.potential)(&t2)) / (t1 - t2);

        let (c_a, c_b) = AiryWaveFunction::calc_c_a_and_c_b(phase, (t1, t2), (wave_func.c_plus, wave_func.c_minus), u_1, x_1);

        AiryWaveFunction { c_a, c_b, u_1, x_1, ts: vec![(t1, t2)] }
    }
}

impl ReToC for AiryWaveFunction {
    fn eval(&self, x: &f64) -> Complex64 {
        let u_1_cube_root = Self::get_u_1_cube_root(self.u_1);
        let ai = self.c_a * Ai(u_1_cube_root * (x - self.x_1));
        let bi = self.c_b * Bi(u_1_cube_root * (x - self.x_1));
        return ai + bi;
    }
}

#[cfg(test)]
mod test {
    use super::*;

}