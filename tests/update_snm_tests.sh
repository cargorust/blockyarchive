#!/bin/bash

exit_code=0

VERSIONS=(1 2 3 17 18 19)

# Encode in all 6 versions
for ver in ${VERSIONS[*]}; do
  echo -n "Encoding in version $ver"
  output=$(./../blkar encode --json --sbx-version $ver -f dummy dummy$ver.sbx \
                      --rs-data 10 --rs-parity 2)
  if [[ $(echo $output | jq -r ".error") != null ]]; then
    echo " ==> Invalid JSON"
    exit_code=1
  fi
  if [[ $(echo $output | jq -r ".stats.sbxVersion") == "$ver" ]]; then
    echo " ==> Okay"
  else
    echo " ==> NOT okay"
    exit_code=1
  fi
done

# Show all
for ver in ${VERSIONS[*]}; do
    echo -n "Checking show output for $ver container"
    output=$(./../blkar show --json --pv 1 dummy$ver.sbx 2>/dev/null)
    if [[ $(echo $output | jq -r ".error") != null ]]; then
        echo " ==> Invalid JSON"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".blocks[0].sbxContainerVersion") == $ver ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".blocks[0].fileName") == "dummy" ]]; then
        echo " ==> Okay"
    else
        echo " ==> NOT okay"
        exit_code=1
    fi
done

new_snm={}

# Change file name
for ver in ${VERSIONS[*]}; do
    echo -n "Changing file name of "dummy$ver.sbx
    new_snm[$ver]=$(cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 5 | head -n 1)
    output=$(./../blkar update --json -y --snm ${new_snm[$ver]} -v dummy$ver.sbx)
    if [[ $(echo $output | jq -r ".error") != null ]]; then
        echo " ==> Invalid JSON"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".stats.sbxVersion") == "$ver" ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".metadataChanges[0].changes[0].field") == "SNM" ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".metadataChanges[0].changes[0].from") == dummy$ver.sbx ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".metadataChanges[0].changes[0].to") == ${new_snm[$ver]} ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".metadataChanges[3]") == "null" ]]; then
        echo -n " ==> Okay"
    else
        echo -n " ==> NOT okay"
        exit_code=1
    fi
    if [[ $(echo $output | jq -r ".metadataChanges[0].changes[1]") == "null" ]]; then
        echo " ==> Okay"
    else
        echo " ==> NOT okay"
        exit_code=1
    fi
done

# Show all
for ver in ${VERSIONS[*]}; do
  echo -n "Checking show output for $ver container"
  output=$(./../blkar show --json --pv 1 dummy$ver.sbx 2>/dev/null)
  if [[ $(echo $output | jq -r ".error") != null ]]; then
    echo " ==> Invalid JSON"
    exit_code=1
  fi
  if [[ $(echo $output | jq -r ".blocks[0].sbxContainerVersion") == $ver ]]; then
    echo -n " ==> Okay"
  else
    echo -n " ==> NOT okay"
    exit_code=1
  fi
  if [[ $(echo $output | jq -r ".blocks[0].sbxContainerName") == ${new_snm[$ver]} ]]; then
      echo " ==> Okay"
  else
      echo " ==> NOT okay"
      exit_code=1
  fi
done

echo $exit_code > exit_code
