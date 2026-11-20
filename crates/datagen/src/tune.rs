//! Logistic fitting of Bear's tapered evaluation weights to self-play outcomes.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use bear_core::Position;
use bear_core::eval::{EvalFeatures, FEATURE_COUNT, MAX_PHASE, Params};

pub const USAGE: &str = "datagen tune --input <selfplay file> --out <weights.rs> [--epochs N]";
type Weights = [[f64; 2]; FEATURE_COUNT];

// Centipawns on an Elo-style logistic curve. Adam updates are in centipawns.
const LOGIT_SCALE: f64 = std::f64::consts::LN_10 / 400.0;
const LEARNING_RATE: f64 = 2.0;
const L2: f64 = 0.000_001;

struct Sample {
    terms: Vec<(usize, i16)>,
    phase: [f64; 2],
    result: f64,
}

impl Sample {
    fn logit(&self, weights: &Weights) -> f64 {
        let [mg, eg] = self.phase;
        self.terms
            .iter()
            .map(|&(term, value)| {
                (weights[term][0] * mg + weights[term][1] * eg) * f64::from(value)
            })
            .sum::<f64>()
            * LOGIT_SCALE
    }
}

fn samples(input: &str) -> Result<(Vec<Sample>, Vec<Sample>), String> {
    let file = File::open(input).map_err(|err| format!("cannot open {input}: {err}"))?;
    let mut lines = BufReader::new(file).lines();
    if lines
        .next()
        .transpose()
        .map_err(|err| err.to_string())?
        .as_deref()
        != Some(super::selfplay::FORMAT)
    {
        return Err(format!("{input} lacks the Bear self-play format header"));
    }
    let mut train = Vec::new();
    let mut validation = Vec::new();
    for (index, line) in lines.enumerate() {
        let line = line.map_err(|err| format!("cannot read {input}: {err}"))?;
        let invalid = || format!("{input}: invalid training record at line {}", index + 2);
        let mut fields = line.splitn(3, '\t');
        let game = fields
            .next()
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(invalid)?;
        let result = fields
            .next()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|&v| v <= 2)
            .ok_or_else(invalid)?;
        let fen = fields.next().ok_or_else(invalid)?;
        let (pos, _) = Position::from_fen(fen).map_err(|_| invalid())?;
        let features = EvalFeatures::new(&pos);
        let phase = f64::from(features.phase) / f64::from(MAX_PHASE);
        let terms = features
            .counts
            .into_iter()
            .enumerate()
            .filter(|&(_, value)| value != 0)
            .collect();
        let sample = Sample {
            terms,
            phase: [phase, 1.0 - phase],
            result: f64::from(result) / 2.0,
        };
        if game.is_multiple_of(5) {
            validation.push(sample);
        } else {
            train.push(sample);
        }
    }
    if train.is_empty() || validation.is_empty() {
        return Err("tuning requires records from both training and held-out games".to_string());
    }
    Ok((train, validation))
}

fn mean_loss(samples: &[Sample], weights: &Weights) -> f64 {
    samples
        .iter()
        .map(|sample| {
            let x = sample.logit(weights);
            x.max(0.0) - sample.result * x + (-x.abs()).exp().ln_1p()
        })
        .sum::<f64>()
        / samples.len() as f64
}

fn fit(train: &[Sample], initial: Weights, epochs: usize) -> [[i16; 2]; FEATURE_COUNT] {
    let mut weights = initial;
    let mut mean = [[0.0; 2]; FEATURE_COUNT];
    let mut variance = [[0.0; 2]; FEATURE_COUNT];
    let mut beta1_power = 1.0;
    let mut beta2_power = 1.0;
    for _ in 0..epochs {
        let mut gradient = [[0.0; 2]; FEATURE_COUNT];
        for sample in train {
            let predicted = 1.0 / (1.0 + (-sample.logit(&weights)).exp());
            let error = (predicted - sample.result) * LOGIT_SCALE;
            for &(term, count) in &sample.terms {
                let contribution = error * f64::from(count);
                gradient[term][0] += contribution * sample.phase[0];
                gradient[term][1] += contribution * sample.phase[1];
            }
        }
        beta1_power *= 0.9;
        beta2_power *= 0.999;
        for term in 0..FEATURE_COUNT {
            for phase in 0..2 {
                let prior = initial[term][phase];
                let old = weights[term][phase];
                let grad = gradient[term][phase] / train.len() as f64 + L2 * (old - prior);
                let avg = &mut mean[term][phase];
                let squared = &mut variance[term][phase];
                *avg = 0.9 * *avg + 0.1 * grad;
                *squared = 0.999 * *squared + 0.001 * grad * grad;
                let corrected_avg = *avg / (1.0 - beta1_power);
                let corrected_squared = *squared / (1.0 - beta2_power);
                let delta = LEARNING_RATE * corrected_avg / (corrected_squared.sqrt() + 1e-8);
                weights[term][phase] = (old - delta).clamp(prior - 96.0, prior + 96.0);
            }
        }
    }
    std::array::from_fn(|term| {
        std::array::from_fn(|phase| {
            let prior = initial[term][phase];
            // A small fit can mistake correlated features for causes. Preserve
            // disabled phases and keep the fitted value near the evaluation prior.
            let limit = (prior.abs() * 0.25).floor().min(16.0);
            let delta = ((weights[term][phase] - prior) * 0.25).clamp(-limit, limit);
            (prior + delta).round() as i16
        })
    })
}

pub fn run(args: &[String]) -> Result<(), String> {
    let (mut input, mut out, mut epochs) = (None, None, 60);
    let mut args = args.iter();
    while let Some(option) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("{option} needs a value"))?;
        match option.as_str() {
            "--input" => input = Some(value),
            "--out" => out = Some(value),
            "--epochs" => {
                epochs = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&epochs| epochs > 0)
                    .ok_or_else(|| format!("invalid epoch count: {value}"))?
            }
            _ => return Err(format!("unknown option {option}\n{USAGE}")),
        }
    }
    let (Some(input), Some(out)) = (input, out) else {
        return Err(USAGE.to_string());
    };
    let (train, validation) = samples(input)?;
    let initial = Params::default().weights().map(|pair| pair.map(f64::from));
    let fitted = fit(&train, initial, epochs);
    let quantized = fitted.map(|pair| pair.map(f64::from));
    let untuned = Params::untuned();
    let write = || -> std::io::Result<()> {
        let mut output = BufWriter::new(File::create(out)?);
        writeln!(
            output,
            "//! Evaluation weights generated by datagen tune.\n"
        )?;
        writeln!(
            output,
            "pub(super) const WEIGHTS: &[(usize, [i16; 2])] = &["
        )?;
        for (term, &[mg, eg]) in fitted.iter().enumerate() {
            if [mg, eg] != untuned.weights()[term] {
                writeln!(output, "    ({term}, [{mg}, {eg}]),")?;
            }
        }
        writeln!(output, "];")?;
        output.flush()
    };
    write().map_err(|err| format!("cannot write {out}: {err}"))?;
    println!(
        "training {} positions: {:.6} -> {:.6} log loss",
        train.len(),
        mean_loss(&train, &initial),
        mean_loss(&train, &quantized)
    );
    println!(
        "held out {} positions: {:.6} -> {:.6} log loss",
        validation.len(),
        mean_loss(&validation, &initial),
        mean_loss(&validation, &quantized)
    );
    println!("wrote tuned weights to {out}");
    Ok(())
}
