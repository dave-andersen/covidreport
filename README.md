# covidreport
Tools for analyzing PA covid data. It's kind of a big spaghetti bundle; making it
available because transparency is important in this kind of reporting.

Consists of two parts: A rust binary (covidreport)
that processes the data, and a shell script (makeplots.sh) that fetches the data.
Has some things hardcoded to Dave's machine, sorry; patches welcome.

```
  sh makeplots.sh
  ./target/release/covidreport
```

Will produce text output used for the daily thread and a set of png files
that are the graphs for that day.
