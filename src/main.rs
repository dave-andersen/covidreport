use anyhow::Result;
use chrono::{Datelike, TimeZone};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use plotters::prelude::*;

use std::{collections::HashMap, iter::Iterator};

const CSVDIR: &str = "/home/dga/pa_data";
const CASES_PREFIX: &str = "daily";
const HOSP_PREFIX: &str = "today";
const PCR_PREFIX: &str = "pcr_test_counts";
const TESTS_DIR: &str = "/home/dga/testday";

/// Importer for [OpendataPA hospitalization data](https://data.pa.gov/Covid-19/COVID-19-Aggregate-Hospitalizations-Current-Daily-/kayn-sjhx)
///
/// Requires the [raw data in CSV format](https://data.pa.gov/api/views/kayn-sjhx/rows.csv?accessType=DOWNLOAD)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HospitalRecord {
    #[serde(rename = "County")]
    county: String,
    #[serde(rename = "Date of data")]
    #[serde(with = "mdY_date_format")]
    date: chrono::NaiveDate,
    #[serde(rename = "Adult ICU Beds Available")]
    adult_icu_beds_available: Option<u32>,
    #[serde(rename = "Adult ICU Beds Total")]
    adult_icu_beds_total: Option<u32>,
    #[serde(rename = "Medical/Surgical Beds Available")]
    med_surg_available: Option<u32>,
    #[serde(rename = "Medical/Surgical Beds Total")]
    med_surg_total: Option<u32>,
    #[serde(rename = "COVID-19 Patients Hospitalized")]
    covid_hospitalized: Option<u32>,
    #[serde(rename = "COVID-19 Patients on Ventilators")]
    covid_ventilator: Option<u32>,
    #[serde(rename = "COVID-ICU")]
    covid_icu: Option<u32>,
    // This field is injected later and is not part of the CSV
    #[serde(skip)]
    new_cases: Option<u32>,
}

/// Importer for [WPRDC test results data](https://data.wprdc.org/dataset/allegheny-county-covid-19-tests-cases-and-deaths)
///
/// Requires the [raw data in CSV format](https://data.wprdc.org/dataset/allegheny-county-covid-19-tests-cases-and-deaths/resource/4051a85a-bf92-45fc-adc6-b31eb8efaad4) (warning, this is a 60+MB download)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestRecord {
    indv_id: String,
    #[serde(with = "Ymd_dash_date_format")]
    collection_date: chrono::NaiveDate,
    #[serde(with = "Ymd_dash_date_format")]
    report_date: chrono::NaiveDate,
    test_result: String,
    case_status: String,
    hospital_flag: String,
    icu_flag: String,
    vent_flag: String,
    age_bucket: String,
    sex: String,
    race: String,
    ethnicity: String,
    #[serde(with = "Ymd_dash_date_format")]
    update_date: chrono::NaiveDate,
}

/// Importer for
/// [OpendataPA cases data](https://data.pa.gov/Covid-19/COVID-19-Aggregate-Cases-Current-Daily-County-Heal/j72v-r42c)
///
/// Requires the [raw data feed in CSV format](https://data.pa.gov/api/views/j72v-r42c/rows.csv?accessType=DOWNLOAD)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CasesRecord {
    #[serde(rename = "Jurisdiction")]
    county: String,
    #[serde(rename = "Date")]
    #[serde(with = "mdY_date_format")]
    date: chrono::NaiveDate,
    #[serde(rename = "New Cases")]
    new_cases: Option<u32>,
}

#[allow(non_snake_case)]
mod mdY_date_format {
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%m/%d/%Y";
    pub fn serialize<S>(nd: &chrono::NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = nd.format(FORMAT);
        serializer.serialize_str(&s.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<chrono::NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        chrono::NaiveDate::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

#[allow(non_snake_case)]
mod Ymd_dash_date_format {
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y-%m-%d";
    pub fn serialize<S>(nd: &chrono::NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = nd.format(FORMAT);
        serializer.serialize_str(&s.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<chrono::NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        chrono::NaiveDate::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

fn csvrecs<T>(filename: &str) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
    T: std::fmt::Debug,
{
    let infile = std::fs::File::open(filename)?;
    let mut rdr = csv::Reader::from_reader(infile);
    Ok(rdr
        .deserialize()
        .filter_map(Option::Some)
        .filter_map(|o| {
            // if o.is_err() {
            //     println!("Error unpacking {:?}", o);
            // }
            o.ok()
        })
        .collect())
}

fn plot_jurisdiction(recs: &[HospitalRecord], jurisdiction: &str, is_60d: bool, y_truncate: bool) -> Result<()> {
    let mut img_path = std::path::PathBuf::from(str::replace(jurisdiction, " ", "_"));
    img_path.set_extension("png");
    let dates: Vec<chrono::NaiveDate> = recs.iter().map(|x| x.date).collect();
    let root = BitMapBackend::new(&img_path, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;
    let cur_cases_style = plotters::style::ShapeStyle {
        color: plotters::style::Palette99::pick(1).mix(0.4).to_rgba(),
        filled: true,
        stroke_width: 2,
    };
    let daily_cases_style = plotters::style::ShapeStyle {
        color: plotters::style::Palette99::pick(3).mix(0.3).to_rgba(),
        filled: true,
        stroke_width: 1,
    };
    let avg_cases_style = plotters::style::ShapeStyle {
            color: plotters::style::Palette99::pick(1).mix(0.9).to_rgba(),
            filled: true,
            stroke_width: 2,
    };
    let hosp_style = plotters::style::ShapeStyle {
        color: plotters::style::Palette99::pick(0).mix(0.9).to_rgba(),
        filled: true,
        stroke_width: 2,
    };
    let icu_style = plotters::style::ShapeStyle {
        color: plotters::style::Palette99::pick(5).mix(0.9).to_rgba(),
        filled: true,
        stroke_width: 2,
    };

    let max_date = *(dates.iter().max().unwrap()) + chrono::Duration::days(1);
    let min_date: chrono::NaiveDate = if is_60d {
        max_date - chrono::Duration::days(61)
    } else {
        chrono::NaiveDate::from_ymd(2020, 10, 1)
    };
    let mut max_y = recs
        .iter()
        .map(|x| x.new_cases.unwrap_or(0))
        .max()
        .unwrap_or(1000);
    max_y += max_y / 20;
    if y_truncate {
        max_y /= 4;
    }
    let casevec: Vec<u32> = recs
        .iter()
        .take(recs.len() - 1)
        .map(|x| x.new_cases.unwrap_or(0))
        .collect();
    let cases7day: Vec<u32> = casevec
        .windows(7)
        .map(|w| ((w.iter().sum::<u32>() as f64) / (w.len() as f64)).round() as u32)
        .collect();
    let latest_cases = cases7day[cases7day.len()-1];

    let dates7day = recs.iter().skip(6).map(|x| x.date).take(cases7day.len());
    let datecases7day = dates7day.zip(cases7day);

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .caption(
            format!("Cases and hospitalizations: {jurisdiction}"),
            ("sans-serif", 40),
        )
        .set_label_area_size(LabelAreaPosition::Left, 60)
        .set_label_area_size(LabelAreaPosition::Right, 60)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .build_cartesian_2d(min_date..max_date, 0u32..max_y)?;
    chart.configure_mesh().bold_line_style(&BLACK.mix(0.10)).light_line_style(&BLACK.mix(0.05)).x_labels(10).x_desc("Date").draw()?;
    chart
        .draw_series(LineSeries::new(
            recs.iter()
                .take(recs.len() - 1)
                .map(|x| (x.date, x.new_cases.unwrap_or(0))),
            daily_cases_style.clone(),
        ))?
        .label("Daily new cases")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], daily_cases_style.clone()));
    chart
        .draw_series(LineSeries::new(datecases7day, avg_cases_style.clone()))?
        .label("7 day avg new cases")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], avg_cases_style.clone()));
    let hosp_cases = cleanup(recs.iter().map(|x| x.covid_hospitalized));
    chart
        .draw_series(LineSeries::new(
            recs.iter().map(|x| x.date).zip(hosp_cases.into_iter()),
            hosp_style.clone(),
        ))?
        .label("Total hospitalized")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], hosp_style.clone()));
    let icu_cases = cleanup(recs.iter().map(|x| x.covid_icu));
    chart
        .draw_series(LineSeries::new(
            recs.iter().map(|x| x.date).zip(icu_cases.into_iter()),
            icu_style.clone(),
        ))?
        .label("ICU beds used")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], icu_style.clone()));

    chart
        .draw_series(LineSeries::new(
        vec![(min_date, latest_cases), (max_date, latest_cases)],
        cur_cases_style.clone(),
        ))?
        .label("Current Cases Level")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], cur_cases_style.to_owned()));
    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;
    Ok(())
}

fn printstats(recs: &[HospitalRecord], icunorm: u32, icunormfree: u32, population: Option<u32>) {
    let last = recs.len() - 1;
    let newh = recs[last].covid_hospitalized.unwrap();
    let prev = recs[last - 1]
        .covid_hospitalized
        .unwrap_or_else(|| recs[last - 2].covid_hospitalized.unwrap()) as i32;
    let hd = newh as i32 - prev as i32;
    //let hd = newh as i32 - recs[last - 1].covid_hospitalized.unwrap() as i32;
    println!("Hospitalizations are {:+} to {newh}  ", hd);
    let newi = recs[last].covid_icu.unwrap();
    let previ = recs[last - 1]
        .covid_icu
        .unwrap_or_else(|| recs[last - 2].covid_icu.unwrap()) as i32;
    let id = newi as i32 - previ;
    // recs[last - 1].covid_icu.unwrap() as i32;
    print!("ICUs are {:+} to {} ", id, newi);
    println!(
        "({:.0}% full)  ",
        ((((icunorm - icunormfree + newi) * 100) as f32) / (icunorm as f32))
    );

    let highest_cases = [0,1].iter().map(|step| {
        let cases_7_day_avg = recs[last - 7 - step..last - step]
            .iter()
            .map(|x| x.new_cases.unwrap_or(0) as f32)
            .sum::<f32>()
            / 7.0;

        println!("Step{} 7 day avg to {:.0} cases/day  ", step, cases_7_day_avg);
        cases_7_day_avg as u32
    }).max().unwrap();

    if let Some(pop) = population {
        let cases_week_100k = ((highest_cases * 7) as f32) / ((pop as f32) / 100000.0);
        let level = match cases_week_100k {
            x if (0.0..10.0).contains(&x) => "Low",
            x if (10.0..50.0).contains(&x) => "Moderate",
            x if (50.0..100.0).contains(&x) => "Substantial",
            x if (100.0..).contains(&x) => "High",
            _ => "ERROR",
        };
        println!("Transmission risk: {level}  ");
    }
}

fn count_cases(filename: &str, jurisdiction: &str) -> Result<u32> {
    Ok(csvrecs::<CasesRecord>(filename)?
        .iter()
        .filter(|x| x.county == jurisdiction)
        .map(|x| x.new_cases.unwrap_or(0))
        .sum())
}

const CSV_DATE_FORMAT: &str = "%Y%m%d";

fn cases_file(day: &chrono::DateTime<chrono::Local>) -> String {
    let datestamp = day.format(CSV_DATE_FORMAT);
    format!("{}/{}_{}.csv", CSVDIR, CASES_PREFIX, datestamp)
}

fn hosps_file(day: &chrono::DateTime<chrono::Local>) -> String {
    let datestamp = day.format(CSV_DATE_FORMAT);
    format!("{}/{}_{}.csv", CSVDIR, HOSP_PREFIX, datestamp)
}

fn tests_file(day: &chrono::DateTime<chrono::Local>) -> String {
    let datestamp = day.format("%m-%d-%Y");
    format!("{}/{}.csv", TESTS_DIR, datestamp)
}

fn pcr_tests_file(day: &chrono::DateTime<chrono::Local>) -> String {
    let datestamp = day.format(CSV_DATE_FORMAT);
    format!("{}/{}_{}.csv", CSVDIR, PCR_PREFIX, datestamp)
}

fn count_case_delta(
    to_date: &chrono::DateTime<chrono::Local>,
    from_date: &chrono::DateTime<chrono::Local>,
    jurisdiction: &str,
) -> Result<i32> {
    let to_file = cases_file(to_date);
    let from_file = cases_file(from_date);

    Ok(count_cases(&to_file, jurisdiction)? as i32 - count_cases(&from_file, jurisdiction)? as i32)
}

fn get_all_records(today: &chrono::DateTime<chrono::Local>) -> Result<Vec<HospitalRecord>> {
    let case_records = csvrecs::<CasesRecord>(&cases_file(today))?;
    let mut casehash = std::collections::HashMap::new();
    for c in case_records {
        casehash.insert(format!("{}{}", c.county, c.date), c);
    }

    let mut all_records = csvrecs::<HospitalRecord>(&hosps_file(today))?;
    for r in &mut all_records {
        let key = format!("{}{}", r.county, r.date);
        if let Some(caserec) = casehash.get(&key) {
            r.new_cases = caserec.new_cases;
        }
    }
    Ok(all_records)
}

fn analyze(
    all_records: &[HospitalRecord],
    jurisdiction: &str,
    jurisdiction_full: &str,
    new_cases: Option<i32>,
    population: Option<u32>,
) {
    let county_records: Vec<HospitalRecord> = all_records
        .iter()
        .filter(|x| x.county == jurisdiction)
        .sorted_by_key(|x| x.date)
        .cloned()
        .collect();

    if let Some(new_cases) = new_cases {
        println!("{jurisdiction} reports {new_cases} new cases.  ");
    }
    let (icunorm, icunormfree) = if jurisdiction == "Pennsylvania" {
        (4200, 1040)
    } else {
        (560, 180)
    };
    printstats(&county_records, icunorm, icunormfree, population);
    //printstats(&county_records, 560, 180);
    if let Err(e) = plot_jurisdiction(&county_records, jurisdiction_full, false, false) {
        println!("Error plotting jurisdiction {}: {:?}", jurisdiction, e);
    }
    let jurisdiction_60d = format!("{}_60days", jurisdiction_full);
    let county_60d = &county_records[county_records.len() - 60..];
    if let Err(e) = plot_jurisdiction(county_60d, &jurisdiction_60d, true, false) {
        println!("Error plotting jurisdiction {}: {:?}", jurisdiction_60d, e);
    }
    let jurisdiction_trunc = format!("{}_trunc", jurisdiction_full);
    if let Err(e) = plot_jurisdiction(&county_records, &jurisdiction_trunc, false, true) {
        println!("Error plotting jurisdiction {}: {:?}", jurisdiction_trunc, e);
    }
}

fn cleanup<I: Iterator<Item = Option<u32>>>(vals: I) -> Vec<u32> {
    let v: Vec<Option<u32>> = vals.collect();
    let mut result = Vec::new();
    for i in 0..v.len() {
        match v[i] {
            Some(x) => result.push(x),
            None => {
                if i == 0 || i == (v.len() - 1) {
                    result.push(0);
                } else {
                    result.push((v[i - 1].unwrap_or(0) + v[i + 1].unwrap_or(0)) / 2);
                }
            }
        }
    }
    result
}

fn reportcovid(today: &chrono::DateTime<chrono::Local>) -> Result<()> {
    let yesterday = *today - chrono::Duration::days(7); // now last week

    // This is all inefficient but we're fast enough, so ignore.
    let new_cases_allegheny = count_case_delta(today, &yesterday, "Allegheny")?;
    let new_cases_state = count_case_delta(today, &yesterday, "Pennsylvania")?;
    let extra_county = "Philadelphia";
    let extra_county_name = "Philadelphia County";
    let new_cases_philly = count_case_delta(today, &yesterday, extra_county)?;

    let all_records = get_all_records(today)?;

    println!();
    println!("## Allegheny County");
    analyze(
        &all_records,
        "Allegheny",
        "Allegheny County",
        Some(new_cases_allegheny),
        Some(1213570),
    );
    println!("CDC Community level: ");
    println!("\n{{{{ allegheny() }}}}\n\n");

    println!("## Pennsylvania");
    analyze(
        &all_records,
        "Pennsylvania",
        "Pennsylvania",
        Some(new_cases_state),
        Some(12964056),
    );

    println!("\n{{{{ pennsylvania() }}}}\n\n");
    analyze(
        &all_records,
        extra_county,
        extra_county_name,
        Some(new_cases_philly),
        Some(1585480),
    );

    let _res = testreport(today);
    println!();

    println!("## Vaccinations");
    println!("Allegheny County daily vaccines move to X 1st+2nd doses, Y booster doses/day (5 day lagged 7 day avg)\n");
    println!("Fully vaccinated %s:\n");
    println!("Allegheny County: X%, Y% of those are boosted, Z% of boosted got 4th dose.");
    println!();
    hospitalizations(&all_records);



    let all_jurisdictions = all_records
        .iter()
        .map(|r| &r.county)
        .unique()
        .map(|r| r.to_string());
    println!("Increases in 7 day avg cases/day from a week ago:");
    println!();
    for j in all_jurisdictions {
        let recs: Vec<HospitalRecord> = all_records
            .iter()
            .filter(|x| x.county == j)
            .sorted_by_key(|x| x.date)
            .cloned()
            .collect();
        let last = recs.len() - 1;
        let mut step0 = 0.0;
        let mut step7 = 0.0;
        let mut step14 = 0.0;
        for step in [0, 7, 14] {
            let cases_7_day_avg = recs[last - 7 - step..last - step]
                .iter()
                .map(|x| x.new_cases.unwrap_or(0) as f32)
                .sum::<f32>()
                / 7.0;
            match step {
                0 => step0 = cases_7_day_avg,
                7 => step7 = cases_7_day_avg,
                14 => step14 = cases_7_day_avg,
                _ => panic!("Unreachable"),
            };
        }
        if step0 > step7 {
            println!(
                "{} rose from {:.1} to {:.1}  (14d {:.1}) ",
                j, step7, step0, step14
            );
        }
    }
    //    println!("All jurisdictions: {:?}", all_jurisdictions);

    Ok(())
}

fn cmpname(o: std::cmp::Ordering) -> &'static str {
    use std::cmp::Ordering;
    match o {
        Ordering::Less => "less",
        Ordering::Equal => "same",
        Ordering::Greater => "more"
    }
}

fn hospitalizations(all_records: &[HospitalRecord]) {
    let mut pa_records = all_records
        .iter()
        .filter(|x| x.county == "Pennsylvania")
        .cloned()
        .collect::<Vec<_>>();
    pa_records.sort_by_key(|r| r.date);
    let t = pa_records.len() - 1;
    let today = &pa_records[t];
    let today_used = today.med_surg_total.unwrap() - today.med_surg_available.unwrap();
    let week1 = &pa_records[t-7];
    let week1_used = week1.med_surg_total.unwrap() - week1.med_surg_available.unwrap();
    let week2 = &pa_records[t-14];
    let week2_used = week2.med_surg_total.unwrap() - week2.med_surg_available.unwrap();
    let week1_change = cmpname(today_used.cmp(&week1_used));
    let week2_change = cmpname(today_used.cmp(&week2_used));

    println!("## Hospitalizations");
    println!("PA counties with full ICU beds due to covid:  ");
    println!();
    println!("PA-wide hospital census: Using {} med/surg beds, {} {} than last week, {} {} than 2 weeks ago.",
        today_used, today_used.abs_diff(week1_used) , week1_change, today_used.abs_diff(week2_used), week2_change);
    println!();

}

fn dayreport() -> Result<()> {
    println!("Day report");
    let today = chrono::Local::now();
    let mut all_records: Vec<HospitalRecord> = get_all_records(&today)?
        .iter()
        .filter(|x| x.county == "Pennsylvania")
        .cloned()
        .collect();
    all_records.sort_by_key(|r| r.date);
    let nr = all_records.len();
    let num_windows = 16; // analyze 12 weeks of data
    let analysis_length = num_windows * 7;
    let mut dayper: Vec<f32> = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    all_records[nr - analysis_length - 1..nr - 1]
        .chunks(7)
        .for_each(|window| {
            let tot_cases = window
                .iter()
                .map(|r| r.new_cases.unwrap_or(0) as f32)
                .sum::<f32>();
            for w in window {
                let wd = w.date.weekday().num_days_from_monday() as usize; // mon = 0
                println!("w date {} nc: {:?}", w.date, w.new_cases);
                dayper[wd] +=
                    (w.new_cases.unwrap_or(0) as f32) / (tot_cases as f32 * num_windows as f32);
            }
        });
    println!("Dayper: {:?}", dayper);
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "covidreport", about = "Analyze data from PA covid feeds")]
struct Opt {
    #[structopt(short, long)]
    dayreport: bool,
    #[structopt(short, long)]
    agereport: bool,
    #[structopt(long, help = "Analyze for specified date (%Y-%m-%d format)")]
    date: Option<String>,
}

fn get_all_testday_records(day: &chrono::DateTime<chrono::Local>) -> Result<Vec<TestRecord>> {
    let fname = tests_file(day);
    let case_records = csvrecs::<TestRecord>(&fname)?;
    Ok(case_records)
}

fn plot_ages(recs: &[TestRecord], truncate: bool) -> Result<()> {
    let (img_path, max_cases) = if truncate {
        ("case_ages_truncated.png", 150)
    } else { ("case_ages.png", 750)
        };
    let dates: Vec<chrono::NaiveDate> = recs.iter().map(|x| x.report_date).collect();
    let root = BitMapBackend::new(&img_path, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;
    let max_date = *(dates.iter().max().unwrap()) + chrono::Duration::days(1);
    let min_date = *(dates.iter().min().unwrap());
    let ndays = (max_date - min_date).num_days();

    // Create vectors of cases/day grouped by age group;
    // need to partition by age group and then sum counts by day where it's a case

    let mut agebins: HashMap<String, Vec<u32>> = HashMap::new();
    let ages = vec![
        "0 to 9", "10 to 19", "20 to 29", "30 to 39", "40 to 49", "50 to 59", "60 to 69", "70+",
        "unknown",
    ];

    for age in &ages {
        agebins.insert(age.to_string(), vec![0; (ndays + 1) as usize]);
    }
    for rec in recs
        .iter()
        .filter(|x| x.case_status == "Probable" || x.case_status == "Confirmed")
    {
        let case_nday = (rec.report_date - min_date).num_days();
        agebins.get_mut(&rec.age_bucket.to_lowercase()).unwrap()[case_nday as usize] += 1;
    }

    let dates7day = (7..ndays + 1).map(|x| min_date + chrono::Duration::days(x));

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .caption(
            "Cases by age group and date: Allegheny County",
            ("sans-serif", 40),
        )
        .set_label_area_size(LabelAreaPosition::Left, 60)
        .set_label_area_size(LabelAreaPosition::Right, 60)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .build_cartesian_2d(min_date..max_date, 0u32..max_cases)?;
    chart.configure_mesh().x_labels(9).x_desc("Date").draw()?;
    for (color, age) in ages.iter().take(8).enumerate() {
        let cases7day: Vec<u32> = agebins
            .get(age.to_owned())
            .unwrap()
            .windows(7)
            .map(|w| ((w.iter().sum::<u32>() as f64) / (w.len() as f64)).round() as u32)
            .collect();
        let datecases7day = dates7day.to_owned().zip(cases7day);
        let style = plotters::style::ShapeStyle {
            color: plotters::style::Palette99::pick(color).to_rgba(),
            filled: true,
            stroke_width: 1,
        };
        chart
            .draw_series(LineSeries::new(datecases7day, style.clone()))?
            .label(age.to_owned())
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], style.clone()));
    }
    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;

    Ok(())
}

fn agereport(today: &chrono::DateTime<chrono::Local>) -> Result<()> {
    println!("Calculating age report!");
    let mut all_records: Vec<TestRecord> = get_all_testday_records(today)?
        .iter()
        .filter(|x| x.report_date >= chrono::NaiveDate::from_ymd(2021, 1, 1))
        .cloned()
        .collect();
    all_records.sort_by_key(|x| x.report_date);
    plot_ages(&all_records, false)?;
    plot_ages(&all_records, true)
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug, Deserialize)]
struct PCR_tests {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "New PCR Tests")]
    new_tests: i64,
}

fn count_tests(tests_file: &str) -> Result<i64> {
    Ok(csvrecs::<PCR_tests>(tests_file)?
        .iter()
        .map(|x| x.new_tests)
        .sum())
}

fn testreport(today: &chrono::DateTime<chrono::Local>) -> Result<()> {
    let yesterday = *today - chrono::Duration::days(1);
    let yesterday_tests = count_tests(&pcr_tests_file(&yesterday))?;
    let today_tests = count_tests(&pcr_tests_file(today))?;
    println!("Today's results reflect {} new PCR test results", today_tests - yesterday_tests);
    Ok(())
}

fn main() {
    let opt = Opt::from_args();
    let today = if let Some(datestr) = opt.date {
        let n = chrono::NaiveDate::parse_from_str(&datestr, "%Y-%m-%d").unwrap();
        let n = n.and_time(chrono::NaiveTime::from_hms_milli(12, 34, 56, 789));
        chrono::Local.from_local_datetime(&n).unwrap()
    } else {
        chrono::Local::now()
    };
    if opt.agereport {
        if let Err(e) = agereport(&today) {
            println!("Error creating agereport: {}", e);
        }
        return;
    }
    if opt.dayreport {
        if let Err(e) = dayreport() {
            println!("Error creating dayreport: {}", e);
        };
        return;
    }
    let todaystr = today.format("%Y-%m-%d");
    println!("+++\ntitle = \"{todaystr}\"");
    println!("date = {todaystr}");
    println!("+++\n");
    println!("# Allegheny County & Pennsylvania #covid hospitalization & variants thread for {}\n", todaystr);
    let res = reportcovid(&today);
    println!("Res: {:#?}", res);
    let _res = testreport(&today);
    let _res = agereport(&today);
}
