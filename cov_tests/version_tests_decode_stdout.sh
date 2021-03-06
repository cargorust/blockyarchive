#!/bin/bash

source kcov_blkar_fun.sh

exit_code=0

VERSIONS=(1 17)

# Encode in all 6 versions
for ver in ${VERSIONS[*]}; do
  echo -n "Encoding in version $ver"
  output=$(blkar encode --json --sbx-version $ver -f dummy dummy$ver.sbx \
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

# Check all of them
for ver in ${VERSIONS[*]}; do
  echo -n "Checking version $ver container"
  output=$(blkar check --json --pv 2 --verbose dummy$ver.sbx 2>/dev/null)
  if [[ $(echo $output | jq -r ".error") != null ]]; then
    echo " ==> Invalid JSON"
    exit_code=1
  fi
  if [[ $(echo $output | jq -r ".stats.numberOfBlocksFailedCheck") == 0 ]]; then
    echo " ==> Okay"
  else
    echo " ==> NOT okay"
    exit_code=1
  fi
done

# Show all
for ver in ${VERSIONS[*]}; do
  echo -n "Checking show output for $ver container"
  output=$(blkar show --json --pv 1 dummy$ver.sbx 2>/dev/null)
  if [[ $(echo $output | jq -r ".error") != null ]]; then
    echo " ==> Invalid JSON"
    exit_code=1
  fi
  if [[ $(echo $output | jq -r ".blocks[0].sbxContainerVersion") == $ver ]]; then
    echo " ==> Okay"
  else
    echo " ==> NOT okay"
    exit_code=1
  fi
done

# Decode all of them
for ver in ${VERSIONS[*]}; do
  echo -n "Decoding version $ver container"
  output=$(blkar decode --json --verbose dummy$ver.sbx - 2>&1 > dummy$ver)
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
  if [[ $(echo $output | jq -r ".stats.recordedHash") == $(echo $output | jq -r ".stats.hashOfOutputFile") ]]; then
    echo " ==> Okay"
  else
    echo " ==> NOT okay"
    exit_code=1
  fi
done

# Compare to original file
for ver in ${VERSIONS[*]}; do
  echo -n "Comparing decoded version $ver container data to original"
  cmp dummy dummy$ver
  if [[ $? == 0 ]]; then
    echo " ==> Okay"
  else
    echo " ==> NOT okay"
    exit_code=1
  fi
done

echo $exit_code > exit_code
