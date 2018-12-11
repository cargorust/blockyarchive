#!/bin/bash

truncate -s 1M dummy

kcov_rsbx encode dummy -f &> /dev/null

VALS=(1 0 -1)

for v in ${VALS[*]}; do
  echo "show using $v skip-to"
  echo "========================================"

  kcov_rsbx show --skip-to="$v" dummy.sbx

  echo ""
done

for v in ${VALS[*]}; do
    echo "show using $v to"
    echo "========================================"

    kcov_rsbx show --to="$v" dummy.sbx

    echo ""
done
