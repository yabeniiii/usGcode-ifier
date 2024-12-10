use clap::Parser;
use g_code::parse::snippet_parser;
use roxmltree::{self, ParsingOptions};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
};
use svg2gcode::{
    self, svg2program, ConversionConfig, ConversionOptions, Machine, SupportedFunctionality,
};
use svgtypes;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    input_path: std::path::PathBuf,

    /// Decimal number representing scale up or down of input data. Example: 'usGcode -s0.5 input.svg output.gcode' will produce gcode at half scale
    #[arg(short, long)]
    scale: Option<f64>,

    output_path: std::path::PathBuf,
}

fn sanitise_string(s: &str) -> String {
    let mut os: String = String::new();
    for c in s.chars() {
        if c.is_numeric() || c == '.' {
            os.push(c);
        }
    }
    return os;
}

fn main() {
    let args = Args::parse();
    dbg!(&args);

    let svg_file = fs::File::open(&args.input_path);
    let mut svg_xml: String = String::new();
    let _ = match svg_file {
        Ok(mut file) => file.read_to_string(&mut svg_xml),
        Err(err) => panic!(
            "Could not open svg file: {}, failed with error: {}",
            args.input_path.display(),
            err
        ),
    };

    let doc: roxmltree::Document<'_> = match roxmltree::Document::parse_with_options(
        svg_xml.as_str(),
        ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) {
        Ok(doc) => doc,
        Err(err) => panic!(
            "Could not parse svg file: {}, failed with error: {}",
            args.input_path.display(),
            err
        ),
    };
    dbg!(&doc);

    let scaling_factor = match args.scale {
        Some(scale) => scale,
        None => 1.0,
    };

    let doc_width = doc.root().first_child().unwrap().attribute("width");
    let doc_height = doc.root().first_child().unwrap().attribute("height");
    dbg!(doc_width);
    dbg!(doc_height);

    let mut dimensions: [Option<svgtypes::Length>; 2] = [None, None];

    if doc_width.is_some() && doc_height.is_some() {
        dimensions = [
            Some(svgtypes::Length {
                number: (sanitise_string(doc_width.unwrap()).parse::<f64>().unwrap()
                    * scaling_factor),
                unit: svgtypes::LengthUnit::Mm,
            }),
            Some(svgtypes::Length {
                number: (sanitise_string(doc_height.unwrap()).parse::<f64>().unwrap()
                    * scaling_factor),
                unit: svgtypes::LengthUnit::Mm,
            }),
        ]
    }

    let conversion_config = ConversionConfig {
        tolerance: 0.001,
        feedrate: 1000.0,
        dpi: 100.0,
        origin: [Some(0.0), Some(0.0)],
    };

    let machine = Machine::new(
        SupportedFunctionality {
            circular_interpolation: false,
        },
        Some(snippet_parser("M3 G0 Z0.0").expect("Could not parse tool start snippet")),
        Some(snippet_parser("M5 G0 Z3.0").expect("Could not parse tool stop snippet")),
        None,
        None,
    );

    let conversion_options = ConversionOptions {
        dimensions: dimensions,
    };

    let gcode = svg2program(&doc, &conversion_config, conversion_options, machine);
    // dbg!(&gcode);

    match args.output_path.parent() {
        Some(parent) => match fs::create_dir_all(parent) {
            Ok(_) => (),
            Err(err) => panic!(
                "Could not create output file's parent directory(ies), faile with error: {}",
                err
            ),
        },
        None => (),
    };

    match args.output_path.try_exists() {
        Ok(exists) => match exists {
            true => fs::remove_file(&args.output_path)
                .expect("Failed to remove existing file at provided output path"),
            false => {}
        },
        Err(err) => panic!("{}", err),
    };

    let mut output_file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(args.output_path)
    {
        Ok(output) => output,
        Err(err) => panic!(
            "Could not create/open output file, failed with error: {}",
            err
        ),
    };

    for line in gcode.iter() {
        if line.to_string().starts_with("X")
            || line.to_string().starts_with("Y")
            || line.to_string().starts_with("Z")
            || line.to_string().starts_with("F")
        {
            if let Err(err) = write!(output_file, " {}", line.to_string()) {
                panic!("Couldn't write to file: {}", err);
            }
        } else if line.to_string().starts_with(";") {
            // ignore comments
        } else {
            if let Err(err) = write!(output_file, "\n{}", line.to_string()) {
                panic!("Couldn't write to file: {}", err);
            }
        }
    }

    println!(
        "Successfully created gcode at: {}",
        args.output_path.display()
    );
}
