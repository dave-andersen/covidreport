use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use plotters::prelude::*;

use std::iter::Iterator;

const CSVDIR: &str = "/home/dga/pa_data";
const CASES_PREFIX: &str = "daily";
const HOSP_PREFIX: &str = "today";

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
        let s = format!("{}", nd.format(FORMAT));
        serializer.serialize_str(&s)
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
{
    let infile = std::fs::File::open(filename)?;
    let mut rdr = csv::Reader::from_reader(infile);
    Ok(rdr
        .deserialize()
        .filter_map(Option::Some)
        .map(|o| o.unwrap())
        .collect())
}

fn plot_jurisdiction(recs: &[HospitalRecord], jurisdiction: &str) -> Result<()> {
    let mut img_path = std::path::PathBuf::from(jurisdiction);
    img_path.set_extension("png");
    let dates: Vec<chrono::NaiveDate> = recs.iter().map(|x| x.date).collect();
    let root = BitMapBackend::new(&img_path, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;
    let max_date = *(dates.iter().max().unwrap());
    let min_date = chrono::NaiveDate::from_ymd(2020, 10, 1); // dates.iter().min().unwrap();
    let max_y = recs
        .iter()
        .map(|x| x.new_cases.unwrap_or(0))
        .max()
        .unwrap_or(1000);
    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .caption(
            format!("Cases and hospitalizations: {}", jurisdiction),
            ("sans-serif", 40),
        )
        .set_label_area_size(LabelAreaPosition::Left, 60)
        .set_label_area_size(LabelAreaPosition::Right, 60)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .build_cartesian_2d(min_date..max_date, 0u32..max_y)?;
    chart.configure_mesh().x_labels(7).x_desc("Date").draw()?;
    chart
        .draw_series(LineSeries::new(
            recs.iter()
                .take(recs.len() - 1) // Skip bad last cases
                .map(|x| (x.date, x.new_cases.unwrap_or(0))),
            &RED,
        ))?
        .label("Daily new cases")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));
    chart
        .draw_series(LineSeries::new(
            recs.iter()
                .map(|x| (x.date, x.covid_hospitalized.unwrap_or(0))),
            &BLUE,
        ))?
        .label("Total hospitalized")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    chart
        .draw_series(LineSeries::new(
            recs.iter().map(|x| (x.date, x.covid_icu.unwrap_or(0))),
            &BLACK,
        ))?
        .label("ICU beds used")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLACK));
    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;
    Ok(())
}

fn printstats(recs: &[HospitalRecord], icunorm: u32, icunormfree: u32) {
    let last = recs.len() - 1;
    let newh = recs[last].covid_hospitalized.unwrap();
    let hd = newh as i32 - recs[last - 1].covid_hospitalized.unwrap() as i32;
    println!("Hospitalizations are {:+} to {}.", hd, newh);
    let newi = recs[last].covid_icu.unwrap();
    let id = newi as i32 - recs[last - 1].covid_icu.unwrap() as i32;
    print!("ICUs are {:+} to {} ", id, newi);
    println!(
        "({:.0}% full).",
        ((((icunorm - icunormfree + newi) * 100) as f32) / (icunorm as f32))
    );
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

fn count_case_delta(
    to_date: &chrono::DateTime<chrono::Local>,
    from_date: &chrono::DateTime<chrono::Local>,
    jurisdiction: &str,
) -> Result<i32> {
    let to_file = cases_file(to_date);
    let from_file = cases_file(from_date);

    Ok(count_cases(&to_file, jurisdiction)? as i32 - count_cases(&from_file, jurisdiction)? as i32)
}

fn reportcovid() -> Result<()> {
    let today = chrono::Local::now();
    let yesterday = today - chrono::Duration::days(1);

    // This is all inefficient but we're fast enough, so ignore.
    let new_cases_allegheny = count_case_delta(&today, &yesterday, "Allegheny").unwrap();
    let new_cases_state = count_case_delta(&today, &yesterday, "Pennsylvania").unwrap();

    let case_records = csvrecs::<CasesRecord>(&cases_file(&today))?;
    let mut casehash = std::collections::HashMap::new();
    for c in case_records {
        //println!("Case: {:?}", c);
        casehash.insert(format!("{}{}", c.county, c.date), c);
    }
    let mut all_records = csvrecs::<HospitalRecord>(&hosps_file(&today))?;
    for r in &mut all_records {
        let key = format!("{}{}", r.county, r.date);
        if let Some(caserec) = casehash.get(&key) {
            r.new_cases = caserec.new_cases;
        }
    }
    let county_records: Vec<HospitalRecord> = all_records
        .iter()
        .filter(|x| x.county == "Allegheny")
        .sorted_by_key(|x| x.date)
        .cloned()
        .collect();

    println!("County reports {} new cases. ", new_cases_allegheny);
    printstats(&county_records, 560, 180);

    let state_records: Vec<HospitalRecord> = all_records
        .iter()
        .filter(|x| x.county == "Pennsylvania")
        .sorted_by_key(|x| x.date)
        .cloned()
        .collect();

    println!("\nState reports {} new cases. ", new_cases_state);
    printstats(&state_records, 4200, 1040);
    println!("");

    if let Err(e) = plot_jurisdiction(&county_records, "Allegheny County") {
        println!("Error plotting county: {:?}", e);
    }
    if let Err(e) = plot_jurisdiction(&state_records, "Pennsylvania") {
        println!("Error plotting state: {:?}", e);
    }
    //    for r in county_records {
    //println!("L: {:#?}", r);    //}
    Ok(())
}

fn main() {
    let res = reportcovid();
    println!("Res: {:#?}", res);
}
