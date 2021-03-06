use serde_json::json;
use std::env;
use std::path::PathBuf;
use zkp_toolkit::math::Curve;

mod circuits;
use circuits::{CliCircuit, Publics};

use circuits::hash::Hash;
use circuits::mini::Mini;

const PROOFS_DIR: &'static str = "./proof_files";
const SETUP_DIR: &'static str = "./setup_files";

macro_rules! handle_circuit {
    ($curve:ident, $curve_name:expr, $scheme:expr, $circuit:expr, $args:expr) => {
        match $circuit {
            "mini" => {
                let (c, publics) = Mini::<<$curve as Curve>::Fr>::power_on($args);
                let off_c = Mini::<<$curve as Curve>::Fr>::power_off();
                handle_scheme!($curve, c, off_c, publics, $curve_name, $scheme, $circuit);
            }
            "hash" => {
                let (c, publics) = Hash::<<$curve as Curve>::Fr>::power_on($args);
                let off_c = Hash::<<$curve as Curve>::Fr>::power_off();
                handle_scheme!($curve, c, off_c, publics, $curve_name, $scheme, $circuit);
            }
            _ => return Err(format!("CIRCUIT: {} not implement.", $circuit)),
        };
    };
}

macro_rules! handle_scheme {
    ($curve:ident, $c:expr, $off_c:expr, $publics:expr, $curve_name:expr, $scheme:expr, $circuit:expr) => {
        let mut pk_path = PathBuf::from(SETUP_DIR);
        pk_path.push(format!("{}-{}-{}.pk", $scheme, $curve_name, $circuit));
        let pk = std::fs::read(&pk_path).unwrap_or(vec![]);
        let rng = &mut rand::thread_rng();

        let proof_bytes = match $scheme {
            "groth16" => {
                println!("Will use pk file: {:?}", pk_path);
                use zkp_toolkit::groth16::{create_random_proof, Parameters};
                let params: Parameters<$curve> = postcard::from_bytes(&pk).unwrap();
                let proof = create_random_proof(&params, $c, rng).unwrap();
                postcard::to_allocvec(&proof).unwrap()
            }
            "bulletproofs" => {
                use zkp_toolkit::bulletproofs::create_random_proof;
                let (gens, r1cs, proof) = create_random_proof::<$curve, _, _>($c, rng).unwrap();
                let mut gens_bytes = postcard::to_allocvec(&gens).unwrap();
                let mut r1cs_bytes = postcard::to_allocvec(&r1cs).unwrap();
                let mut proof_bytes = postcard::to_allocvec(&proof).unwrap();
                let mut bytes = vec![];
                bytes.extend(&(gens_bytes.len() as u32).to_le_bytes());
                bytes.append(&mut gens_bytes);
                bytes.extend(&(r1cs_bytes.len() as u32).to_le_bytes());
                bytes.append(&mut r1cs_bytes);
                bytes.append(&mut proof_bytes);

                bytes
            }
            "marlin" => {
                use zkp_toolkit::marlin::{index, create_random_proof, UniversalParams};
                let mut srs_path = PathBuf::from(SETUP_DIR);
                srs_path.push(format!("{}-{}.universal_setup", $scheme, $curve_name));
                println!("Will use universal setup file: {:?}", srs_path);
                let srs_bytes = std::fs::read(&srs_path).unwrap_or(vec![]);
                let srs: UniversalParams<$curve> = postcard::from_bytes(&srs_bytes).unwrap();
                let (ipk, _ivk) = index(&srs, $off_c).unwrap();
                let proof = create_random_proof(&ipk, $c, rng).unwrap();
                postcard::to_allocvec(&proof).unwrap()

            }
            "spartan_snark" => {
                use zkp_toolkit::spartan::snark::{Parameters, create_random_proof};
                let mut srs_path = PathBuf::from(SETUP_DIR);
                srs_path.push(format!("{}-{}-{}.universal_setup", $scheme, $curve_name, $circuit));
                println!("Will use universal setup file: {:?}", srs_path);
                let srs_bytes = std::fs::read(&srs_path).unwrap_or(vec![]);
                let srs: Parameters<$curve> = postcard::from_bytes(&srs_bytes).unwrap();
                let (pk, _vk) = srs.keypair();
                let proof = create_random_proof(&pk, $c, rng).unwrap();
                postcard::to_allocvec(&proof).unwrap()
            }
            "spartan_nizk" => {
                use zkp_toolkit::spartan::nizk::{Parameters, create_random_proof};
                let mut srs_path = PathBuf::from(SETUP_DIR);
                srs_path.push(format!("{}-{}-{}.universal_setup", $scheme, $curve_name, $circuit));
                println!("Will use universal setup file: {:?}", srs_path);
                let srs_bytes = std::fs::read(&srs_path).unwrap_or(vec![]);
                let srs: Parameters<$curve> = postcard::from_bytes(&srs_bytes).unwrap();
                let (pk, _vk) = srs.keypair();
                let proof = create_random_proof(&pk, $c, rng).unwrap();
                postcard::to_allocvec(&proof).unwrap()
            }
            _ => return Err(format!("SCHEME: {} not implement.", $scheme)),
        };

        let mut path = PathBuf::from(PROOFS_DIR);
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        path.push(format!("{}-{}-{}.proof.json", $scheme, $curve_name, $circuit));
        println!("Proof file: {:?}", path);

        let params = match $publics {
            Publics::Mini(z) => vec![format!("{}", z)],
            Publics::Hash(image) => vec![
                format!("{}", to_hex(&postcard::to_allocvec(&image).unwrap()))
            ]
        };

        let content = json!({
            "circuit": $circuit,
            "scheme": $scheme,
            "curve": $curve_name,
            "params": params,
            "proof": to_hex(&proof_bytes)
        });
        serde_json::to_writer(&std::fs::File::create(path).unwrap(), &content).unwrap();
    };
}

fn to_hex(v: &[u8]) -> String {
    let mut s = String::with_capacity(v.len() * 2);
    s.extend(v.iter().map(|b| format!("{:02x}", b)));
    s
}

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("zkp-prove");
        println!("");
        println!("Usage: zkp-prove [SCHEME] [CURVE] [CIRCUIT] [ARGUMENTS]");
        println!("");
        println!("SCHEME:");
        println!("    groth16       -- Groth16 zero-knowledge proof system.");
        println!("    bulletproofs  -- Bulletproofs zero-knowledge proof system.");
        println!("    marlin        -- Marlin zero-knowledge proof system.");
        println!("    spartan_snark -- Spartan with snark zero-knowledge proof system.");
        println!("    spartan_nizk  -- Spartan with nizk zero-knowledge proof system.");
        println!("");
        println!("CURVE:");
        println!("    bn_256    -- BN_256 pairing curve.");
        println!("    bls12_381 -- BLS12_381 pairing curve.");
        println!("");
        println!("CIRCUIT:");
        println!("    mini    -- Mini circuit. proof: x * (y + 2) = z.");
        println!("    hash    -- Hash circuit. proof: mimc hash.");
        println!("");
        println!("CIRCUIT ARGUMENTS:");
        println!("    [arguments]    -- circuits arguments.");
        println!("");
        println!("");

        return Err("Params invalid!".to_owned());
    }

    let (curve, scheme, circuit) = (args[2].as_str(), args[1].as_str(), args[3].as_str());

    match curve {
        "bn_256" => {
            use zkp_toolkit::bn_256::Bn_256;
            handle_circuit!(Bn_256, curve, scheme, circuit, &args[4..]);
        }
        "bls12_381" => {
            use zkp_toolkit::bls12_381::Bls12_381;
            handle_circuit!(Bls12_381, curve, scheme, circuit, &args[4..]);
        }
        _ => return Err(format!("Curve: {} not implement.", curve)),
    }

    Ok(())
}
