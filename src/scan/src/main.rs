#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use diap_store::{DiapStore};
use id_store::{IdStore};
use std::path::Path;
use std::time::Instant;

// use regex::Regex;
use std::collections::HashSet;
use autocatalog;

use term::Term;

use structopt::StructOpt;
use std::path::PathBuf;

// ============================================================================
// ============================================================================

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to toml config file
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    /// Path to toml config file for rabbitmq connection
    #[structopt(long, parse(from_os_str))]
    rmq: PathBuf,

    /// scan skip
    #[structopt(short, long)]
    scan_skip: bool,

    /// collect skip
    #[structopt(short, long)]
    collect_skip: bool,

    /// autocatalog skip
    #[structopt(short, long)]
    autocatalog_skip: bool,

    // /// extract bmw
    // #[structopt(short, long)]
    // extract_bmw: bool,

    // /// convert
    // #[structopt(long)]
    // conver: bool,
}
use settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init_timed();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }

    let opt = Opt::from_args();

    let settings = Settings::new(&opt.config).map_err(|err| anyhow!("{:?}: {}", opt.config, err))?;
    println!("config: {:?}, settings: {}", opt.config, settings.as_string_pretty()?);

    let settings_rmq = rmq::Settings::new(&opt.rmq).map_err(|err| anyhow!("{:?}: {}", opt.rmq, err))?;
    println!("rmq: {:?}, settings: {}", opt.config, settings_rmq.as_string_pretty()?);

    let out_dir = PathBuf::from(&settings.out_dir);
// if opt.convert {
//
// } else {
    let pool = rmq::get_pool(settings_rmq)?;
    let queue_uuid = uuid::Uuid::new_v4();
    let queue_name = format!("response-{}", queue_uuid);
    let client_provider = client::Provider::new(client::Kind::ViaProxy(pool.clone(), queue_name));

if !opt.scan_skip {
    let params = settings.params;
    let id_store_file_spec= {
        let mut id_store_file_spec = out_dir.clone();
        id_store_file_spec.push("ids.json");
        id_store_file_spec
    };
    let id_fresh_duration = chrono::Duration::minutes(settings.id_fresh_duration_mins);

    let id_store = IdStore::from_file(&id_store_file_spec).await;
    let mut id_store = match id_store {
        Ok(id_store) => id_store, 
        Err(_) => IdStore::new(),
    };

    let thread_limit_network = settings.thread_limit_network;
    let thread_limit_file = settings.thread_limit_file;
    //

    let mut auth = if let Some(key) = settings.auth_key {
        auth::Lazy::new(auth::Arg::new_ready(key))
    } else if let Some(url) = settings.auth_url {
        auth::Lazy::new(auth::Arg::new_lazy(url))
    } else {
        bail!("nor auth_key, neighter auth_url is specifed in settings");
    };

    let ids = match id_store.get_ids(&params, id_fresh_duration) {
        Some(item) => {
            println!("{} ids loaded from {:?}", item.ret.len(), id_store_file_spec.to_string_lossy());
            &item.ret
        },
        None => {

            let count_limit = settings.count_limit;
            let diaps_count = settings.diaps_count;
            let price_max_inc = settings.price_max_inc;
            let diap_fresh_duration = chrono::Duration::minutes(settings.diap_fresh_duration_mins);

            let diap_store_file_spec = {
                let mut diap_store_file_spec = out_dir.clone();
                diap_store_file_spec.push("diaps.json");
                diap_store_file_spec
            };

            let diaps_arg = diaps::Arg {
                params: &params,
                count_limit,
                diaps_count,
                price_max_inc,
                client_provider: client::Provider::new(client::Kind::Reqwest(3)),
            };

            let diap_store = DiapStore::from_file(&diap_store_file_spec).await;
            let mut diap_store = match diap_store {
                Ok(diap_store) => {
                    trace!("diap_store exists at: {:?}", diap_store_file_spec);
                    diap_store
                }, 
                Err(_) => {
                    trace!("no diap_store at: {:?}", diap_store_file_spec);
                    DiapStore::new()
                },
            };
            let diaps_ret = match diap_store.get_diaps(&diaps_arg, diap_fresh_duration) {
                Some(item) => &item.ret,
                None => {
                    let mut term = Term::init(term::Arg::new().header("Определение диапазонов цен . . ."));
                    let start = Instant::now();
                    let diaps_ret = diaps::get(
                        &mut auth,
                        diaps_arg.clone(), 
                        Some(|arg: diaps::CallbackArg| -> Result<()> {
                            term.output(format!("count_total: {}, checks_total: {}, elapsed_millis: {}, per_millis: {}, detected_diaps: {}, price: {}..{}/{}, count: {}", 
                                arg.count_total,
                                arg.checks_total,
                                arrange_millis::get(arg.elapsed_millis),
                                arrange_millis::get(arg.per_millis),
                                arg.diaps_detected,
                                if arg.price_min.is_none() { "".to_owned() } else { arg.price_min.unwrap().to_string() },
                                if arg.price_max.is_none() { "".to_owned() } else { arg.price_max.unwrap().to_string() },
                                if arg.price_max_delta.is_none() { "".to_owned() } else { arg.price_max_delta.unwrap().to_string() },
                                if arg.count.is_none() { "".to_owned() } else { arg.count.unwrap().to_string() },
                            ));
                            Ok(())
                        })
                    ).await?;
                    let diaps_len = diaps_ret.diaps.len();
                    diap_store.set_diaps(&diaps_arg, diaps_ret);
                    diap_store.to_file(&diap_store_file_spec).await?;
                    println!("{}, Определены диапазоны ({}) цен и записаны в {:?} ", arrange_millis::get(Instant::now().duration_since(start).as_millis()), diaps_len, diap_store_file_spec.to_string_lossy());
                    &diap_store.get_diaps(&diaps_arg, diap_fresh_duration).unwrap().ret
                },
            };

            let items_per_page = settings.items_per_page;

            let ids_arg = ids::Arg {
                params: &params,
                diaps_ret: &diaps_ret, 
                thread_limit_network,
                items_per_page,
                client_provider: client_provider.clone(),
            };
            let mut term = Term::init(term::Arg::new().header("Получение списка идентификаторов . . ."));
            let start = Instant::now();
            let ids_ret = ids::get(&mut auth, ids_arg, Some(|arg: ids::CallbackArg| -> Result<()> {
                term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}, ids_len: {}", 
                    arrange_millis::get(arg.elapsed_millis), 
                    arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                    arrange_millis::get(arg.remained_millis), 
                    arrange_millis::get(arg.per_millis), 
                    arg.elapsed_qt,
                    arg.elapsed_qt + arg.remained_qt,
                    arg.remained_qt,
                    arg.ids_len,
                ));
                Ok(())
            })).await?;
            let ids_len = ids_ret.len();
            id_store.set_ids(&params, ids_ret);
            id_store.to_file(&id_store_file_spec).await?;

            println!("{}, Список идентификаторов ({}) получен и записан в {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ids_len, id_store_file_spec.to_string_lossy());

            &id_store.get_ids(&params, id_fresh_duration).unwrap().ret
        },
    };

    let mut term = Term::init(term::Arg::new().header("Получение объявлений . . ."));
    let arg = cards::Arg {
        ids: &ids, 
        out_dir: &out_dir,
        thread_limit_network,
        thread_limit_file,
        client_provider: client_provider.clone(),
    };
    let start = Instant::now();
    let ret = cards::fetch_and_save(&mut auth, arg, Some(|arg: cards::CallbackArg| -> Result<()> {
        term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
        // println!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
            arrange_millis::get(arg.elapsed_millis), 
            arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
            arrange_millis::get(arg.remained_millis), 
            arrange_millis::get(arg.per_millis), 
            arg.elapsed_qt,
            arg.elapsed_qt + arg.remained_qt,
            arg.remained_qt,
        ));
        Ok(())
    })).await?;
    println!("{}, Объявления получены: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.received_qt);
}

// let records = 
if opt.collect_skip { None } else {
    let cards_dir = {
        let mut cards_dir = out_dir.clone();
        cards_dir.push("cards");
        cards_dir
    };
    let arg = collect::Arg { 
        out_dir: &cards_dir.as_path(),
        thread_limit_file: 3,
    };

    let mut term = Term::init(term::Arg::new().header("Чтение объявлений . . ."));
    let start = Instant::now();
    let mut records = collect::Records::new();
    collect::items(arg, &mut records, Some(|arg: collect::CallbackArg| -> Result<()> {
        match arg {
            collect::CallbackArg::ReadDir {elapsed_millis, dir_qt, file_qt} => {
                term.output(format!("time: {}, dirs: {}, files: {}", 
                    arrange_millis::get(elapsed_millis), 
                    dir_qt,
                    file_qt,
                ));
                Ok(())
            },
            collect::CallbackArg::ReadFile {elapsed_millis, remained_millis, per100_millis, elapsed_qt, remained_qt} => {
                term.output(format!("time: {}/{}-{}, per100: {}, qt: {}/{}-{}", 
                    arrange_millis::get(elapsed_millis), 
                    arrange_millis::get(elapsed_millis + remained_millis), 
                    arrange_millis::get(remained_millis), 
                    arrange_millis::get(per100_millis), 
                    elapsed_qt,
                    elapsed_qt + remained_qt,
                    remained_qt,
                ));
                Ok(())
            },
        }
    })).await?;
    println!("{}, Объявления прочитаны: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), records.0.len());


    if !opt.autocatalog_skip {

        let mut autocatalog_urls: HashSet<String> = HashSet::new();
        for record in records.0.iter() {
            if let Some(autocatalog_url) = &record.autocatalog_url {
                autocatalog_urls.insert(autocatalog_url.to_owned());
            }
        }

        trace!("autocatalog_urls: {}", autocatalog_urls.len());

        let out_dir = Path::new("/out");
        let arg = autocatalog::Arg {
            items_to_check: &autocatalog_urls,
            out_dir,
            thread_limit_network: 50,
            thread_limit_file: 2,
            client_provider: client_provider.clone(),
                // client::Provider::new(client::Kind::ViaProxy(pool.clone(), "autocatalog".to_owned())),
        };

        let mut term = Term::init(term::Arg::new().header("Получение автокаталога . . ."));
        autocatalog::fetch_and_save(arg, Some(|arg: autocatalog::CallbackArg| -> Result<()> {
            term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
                arrange_millis::get(arg.elapsed_millis), 
                arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                arrange_millis::get(arg.remained_millis), 
                arrange_millis::get(arg.per_millis), 
                arg.elapsed_qt,
                arg.elapsed_qt + arg.remained_qt,
                arg.remained_qt,
            ));
            Ok(())
        })).await?;
    }

    for mut record in records.0.iter_mut() {
        if let Some(autocatalog_url) = &record.autocatalog_url {
            match autocatalog::get(&out_dir, autocatalog_url).await {
                Err(err) => error!("autocatalog_url not found: {}", err),
                Ok(autocatalog_record) => {
                    record.autocatalog_id = Some(autocatalog_record.id);
                    record.autocatalog_title = Some(autocatalog_record.title);

                    record.autocatalog_transmission = autocatalog_record.transmission;

                    record.autocatalog_engine_displacement = autocatalog_record.engine_displacement;
                    record.autocatalog_engine_displacement_precise = autocatalog_record.engine_displacement_precise;

                    record.autocatalog_drive = autocatalog_record.drive;
                    record.autocatalog_fuel_type = autocatalog_record.fuel_type;

                    record.autocatalog_engine_power = autocatalog_record.engine_power;
                    record.autocatalog_maximum_speed = autocatalog_record.maximum_speed;
                    record.autocatalog_acceleration = autocatalog_record.acceleration;

                    record.autocatalog_brand_country = autocatalog_record.brand_country;
                    record.autocatalog_assembly_country = autocatalog_record.assembly_country;

                    record.autocatalog_number_of_seats = autocatalog_record.number_of_seats;

                    record.autocatalog_rating = autocatalog_record.rating;

                    record.autocatalog_number_of_cylinders = autocatalog_record.number_of_cylinders;
                    record.autocatalog_configuration = autocatalog_record.configuration;

                    record.autocatalog_torque = autocatalog_record.torque;
                    record.autocatalog_torque_max = autocatalog_record.torque_max;
                    record.autocatalog_max_power_speed = autocatalog_record.max_power_speed;

                    record.autocatalog_height = autocatalog_record.height;
                    record.autocatalog_length = autocatalog_record.length;
                    record.autocatalog_turning_diameter = autocatalog_record.turning_diameter;
                    record.autocatalog_clearance = autocatalog_record.clearance;
                    record.autocatalog_wheelbase = autocatalog_record.wheelbase;
                    record.autocatalog_rear_track = autocatalog_record.rear_track;
                    record.autocatalog_front_track = autocatalog_record.front_track;

                    record.autocatalog_trunk_volume = autocatalog_record.trunk_volume;

                    record.autocatalog_fuel_tank_capacity = autocatalog_record.fuel_tank_capacity;

                    record.autocatalog_fuel_consumption_city = autocatalog_record.fuel_consumption_city;
                    record.autocatalog_fuel_consumption_highway = autocatalog_record.fuel_consumption_highway;
                    record.autocatalog_fuel_consumption_mixed = autocatalog_record.fuel_consumption_mixed;

                    record.autocatalog_environmental_class = autocatalog_record.environmental_class;

                    record.autocatalog_rear_breaks = autocatalog_record.rear_breaks;
                    record.autocatalog_front_breaks = autocatalog_record.front_breaks;

                    record.autocatalog_rear_tire_dimension = autocatalog_record.rear_tire_dimension;
                    record.autocatalog_front_tire_dimension = autocatalog_record.front_tire_dimension;

                    record.autocatalog_rear_suspension = autocatalog_record.rear_suspension;
                    record.autocatalog_front_suspension = autocatalog_record.front_suspension;

                    record.autocatalog_world_premier = autocatalog_record.world_premier;
                    record.autocatalog_pending_update = autocatalog_record.pending_update;
                    record.autocatalog_width_with_mirrors = autocatalog_record.width_with_mirrors;
                    record.autocatalog_rear_disc_dimension = autocatalog_record.rear_disc_dimension;
                    record.autocatalog_front_disc_dimension = autocatalog_record.front_disc_dimension;
                }
            };
        }
    }

    let start = Instant::now();
    let file_path = {
        let mut file_path = out_dir.clone();
        file_path.push("records.csv");
        file_path
    };
    // Path::new("/out/records.csv");
    let arg = to_csv::Arg {
        file_path: &file_path,
        records: &records,
    };
    to_csv::write(arg).await?;
    println!("{}, Записаны в файл {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), file_path);
    Some(records)
};

// if opt.extract_bmw {
//     let records = match records {
//         Some(records) => records,
//         None => {
//             let file_path = {
//                 let mut file_path = out_dir.clone();
//                 file_path.push("records.csv");
//                 file_path
//             };
//             let mut reader = csv::Reader::from_path(file_path)?;
//             let mut records = collect::Records::new();
//             for record in reader.deserialize() {
//                 let record: record::card::Record = record?;
//                 // let record: record::card::Record = record.deserialize();
//                 records.0.push(record);
//             }
//             records
//         },
//     };
//     println!("Отбор BMW из объявлений: {}", records.0.len());
//     let start = std::time::Instant::now();
//     let mut bmw_records = collect::Records::new();
//     for record in records.0.into_iter() {
//         if let Some(brand) = &record.brand {
//             if brand == "BMW" {
//                 bmw_records.0.push(record);
//             }
//         }
//     }
//     let file_path = {
//         let mut file_path = out_dir.clone();
//         file_path.push("records_bmw.csv");
//         file_path
//     };
//     let arg = to_csv::Arg {
//         file_path: &file_path,
//         records: &bmw_records,
//     };
//     to_csv::write(arg).await?;
//     println!("{}, BMW отбраны ({}) и записаны в файл {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), bmw_records.0.len(), file_path);
// }

// }

// if false {
//     let mut reader = csv::Reader::from_path("/out/test.csv")?;
//     let mut count = 0usize;
//     lazy_static::lazy_static! {
//         static ref RE_SUFFIX: Regex = Regex::new(r#"\s*\(\d+ (?:л\.с\.|кВт)\)(?: 4WD)?"#).unwrap();
//         static ref RE_MT_AT: Regex = Regex::new(r#"(?: (?:AM?|M)T)"#).unwrap();
//         static ref RE_XDRIVE: Regex = Regex::new(r#"(?:\s*[xs]Drived?\s*)"#).unwrap();
//         static ref RE_VOL: Regex = Regex::new(r#"(?:\s*\d+\.\d(?:s?d|si|i|is|hyb)?)$"#).unwrap();
//         static ref RE_STEPTRONIC: Regex = Regex::new(r#"(?:\s*Steptronic\s*)"#).unwrap();
//     }
//     let mut match_count = 0usize;
//     let mut models_stripped: HashSet<String> = HashSet::new();
//     let mut models: HashSet<String> = HashSet::new();
//     for record in reader.records() {
//         let record = record?;
//         if let Some(model) = record.get(5) {
//             models.insert(model.to_owned());
//             if RE_SUFFIX.is_match(model) {
//                 let model = RE_SUFFIX.replace(model, "");
//                 let model = RE_MT_AT.replace(&model, "");
//                 let model = RE_XDRIVE.replace(&model, "");
//                 let model = RE_VOL.replace(&model, "");
//                 // let model = RE_VOL2.replace(&model, "");
//                 // let model = RE_STEPTRONIC.replace(&model, "");
//                 models_stripped.insert(model.to_owned().to_string());
//                 match_count += 1;
//             } else {
//                 warn!("{}", model);
//             }
//         } else {
//             warn!("no model");
//         }
//         count += 1;
//     }
//     let mut models_stripped = models_stripped.into_iter().collect::<Vec<String>>();
//     models_stripped.sort();
//     info!("count: {}, match_count: {}, models: {}, models_stipped: {}, models_stripped: {:?}", count, match_count, models.len(), models_stripped.len(), models_stripped);
//
//     let mut reader = csv::Reader::from_path("/out/records.csv")?;
//     let mut match_count = 0usize;
//     let mut models_stripped: HashSet<String> = HashSet::new();
//     let mut models: HashSet<String> = HashSet::new();
//     for record in reader.records() {
//         let record = record?;
//         if let Some(model) = record.get(5) {
//             models.insert(model.to_owned());
//             if RE_SUFFIX.is_match(model) {
//                 let model = RE_SUFFIX.replace(model, "");
//                 let model = RE_MT_AT.replace(&model, "");
//                 let model = RE_XDRIVE.replace(&model, "");
//                 let model = RE_STEPTRONIC.replace(&model, "");
//                 let model = RE_VOL.replace(&model, "");
//                 // let model = RE_VOL2.replace(&model, "");
//                 models_stripped.insert(model.to_owned().to_string());
//                 match_count += 1;
//             } else {
//                 warn!("{}", model);
//             }
//         } else {
//             warn!("no model");
//         }
//         count += 1;
//     }
//     let mut models_stripped = models_stripped.into_iter().collect::<Vec<String>>();
//     models_stripped.sort();
//     info!("count: {}, match_count: {}, models: {}, models_stipped: {}, models_stripped: {:?}", count, match_count, models.len(), models_stripped.len(), models_stripped);
// // }
//

    Ok(())
}

