today=`date +%m/%d/%Y`
today_under=`date +%m-%d-%Y`
cp today.csv yesterday.csv
wget -q 'https://data.pa.gov/api/views/j72v-r42c/rows.csv?accessType=DOWNLOAD' -O daily_raw.csv
wget -q 'https://data.pa.gov/api/views/kayn-sjhx/rows.csv?accessType=DOWNLOAD' -O today.csv
wget -q 'https://data.pa.gov/api/views/bicw-3gwi/rows.csv?accessType=DOWNLOAD' -O pavax.csv
wget -q 'https://data.pa.gov/api/views/niuh-2xe3/rows.csv?accessType=DOWNLOAD' -O vaxday/${today_under}.csv
wget -q 'https://data.wprdc.org/dataset/80e0ca5d-c88a-4b1a-bf5d-51d0aaeeac86/resource/4051a85a-bf92-45fc-adc6-b31eb8efaad4/download/covid_19_testing_cases.csv' -O testday/${today_under}.csv
datestamp=`date +%Y%m%d`
python3 fix_cases.py > daily.csv
cp daily_raw.csv pa_data/daily_${datestamp}_raw.csv
cp daily.csv pa_data/daily_${datestamp}.csv
cp today.csv pa_data/today_${datestamp}.csv
#echo -n "yesterday cases total: " 
awk -F ',' '{sum += $2} END {print sum}' all_cases.txt
xsv select 1,2,5,6,13,36 today.csv | xsv search -s 1 Alleg | xsv select 2,3,4,5,6 | csvsort -c 1 | sed 's/,/ /g' | python3 swapdate.py >  plotme
xsv select 1,2,3 daily.csv | xsv search -s 1 Allegheny | csvsort -c 2 | xsv select 2,3 > all_cases.txt
echo -n "today cases total allegheny: "
awk -F ',' '{sum += $2} END {print sum}' all_cases.txt
echo -n "yesterday cases total state: "
awk -F ' ' '{sum += $2} END {print sum}' state_cases.txt
xsv select 1,2,3 daily.csv | xsv search -s 1 Pennsylvania | csvsort -c 2 | xsv select 2,3  | sed 's/,/ /g' | python3 swapdate.py > state_cases.txt
echo -n "today cases total state: "
awk -F ' ' '{sum += $2} END {print sum}' state_cases.txt
xsv select 1,2,5,6,13,36 today.csv | xsv search -s 1 Pennsylvania | xsv select 2,3,4,5,6 | csvsort -c 1 | sed 's/,/ /g' | python3 swapdate.py > plotstate
xsv search -s 2 ${today} today.csv | xsv select 1,2,32,5,6,13,36  | csvsort -c 3 | xsv select 1,2,4,7 | head -15 > worst_counties_icu
cat all_cases.txt | sed 's/,/ /' | python3 swapdate.py > cases_nov.txt
xsv search Allegheny pavax.csv | xsv search 2021 | csvsort -c 1 > allvax.csv
/bin/rm 7dayvax
for i in 19 18 17 16 15 14 13 12 11 10 9 8 7; do tail -${i} allvax.csv | head -7 | awk -F, '{sum += ($3+$4); boostsum+=$5} END {print sum/NR, boostsum/NR}' >> 7dayvax; done
