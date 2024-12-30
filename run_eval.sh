#!/usr/bin/env bash

datetime=$(date +%F_%H-%M-%S)

echo "Setting up eval $datetime"

outdir_base="evaluation_results/$datetime"
mkdir -p "$outdir_base"

results="$outdir_base/results.md"

scenarios=($(ls evaluations))

attempt_count=1

declare -A test_case_params

test_case_params["claude_sonnet_latest_no_seg"]="--model claude-3-5-sonnet-latest"
test_case_params["claude_sonnet_latest_with_seg"]="--apply-segmentation --model claude-3-5-sonnet-latest"
test_case_params["gpt-4o-mini_no_seg"]="--model gpt-4o-mini"
test_case_params["gpt-4o_with_seg"]="--apply-segmentation --model gpt-4o-mini"
test_case_params["gpt-4o-mini_no_seg"]="--model gpt-4o"
test_case_params["gpt-4o_with_seg"]="--apply-segmentation --model gpt-4o"
test_case_params["gemini-2-flash_no_seg"]="--model gemini-2.0-flash-exp"
test_case_params["gemini-2-flash_with_seg"]="--apply-segmentation --model gemini-2.0-flash-exp"
# test_case_params["gemini-1206-flash_no_seg"]="--model gemini-exp-1206"
# test_case_params["gemini-1206-flash_with_seg"]="--apply-segmentation --model gemini-exp-1206"
test_case_params["gemini-1.5-pro_no_seg"]="--model gemini-1.5-pro"
test_case_params["gemini-1.5-pro_with_seg"]="--apply-segmentation --model gemini-1.5-pro"

echo "# Ghostwriter evaluation results $datetime" > $results
echo "" >> $results
# how many scenarios are there
scenario_count=${#scenarios[@]}
test_case_count=${#test_case_params[@]}
total_tests=$(($scenario_count * $test_case_count * $attempt_count))
echo "There are $scenario_count scenarios and $test_case_count test cases with $attempt_count attempts ($total_tests total tests)." >> $results
echo "There are $scenario_count scenarios and $test_case_count test cases with $attempt_count attempts ($total_tests total tests)."

# Loop over each scenario
for scenario in "${scenarios[@]}"; do

  echo "Running scenario $scenario"

  echo "## Test: $scenario" >> $results
  echo "" >> $results

  # Loop over each test_case_params key
  for case_name in ${!test_case_params[@]}; do
    params=${test_case_params[$case_name]}

    # Append to the results.md file
    echo "### $case_name" >> $results

    for attempt in $(seq 1 $attempt_count); do

      # Create output directory
      outdir=$outdir_base/$scenario/$case_name/$attempt
      mkdir -p $outdir

      # Run the test case
      echo "Running scenario $scenario with params $params attempt $attempt"

      ./target/release/ghostwriter \
        --input-png evaluations/$scenario/input.png \
        --output-file $outdir/result.out \
        --model-output-file $outdir/result.json \
        --save-bitmap $outdir/result.png \
        --save-screenshot $outdir/input.png \
        --no-draw \
        --no-draw-progress \
        --no-loop \
        $params

      # Create a merged image with the new part in red
      if [ -f $outdir/result.png ]; then
        convert \
          \( evaluations/$scenario/input.png -colorspace RGB \) \
          \( $outdir/result.png -type truecolormatte -transparent white -fill red -colorize 100 \) \
          -compose Over \
          -composite \
          $outdir/merged-output.png
      fi

      if [ -f $outdir/merged-output.png ]; then
        echo "<img src='../../$outdir/merged-output.png' border=1 width=200 />" >> $results
      else
        echo "<img src='../../evaluations/$scenario/input.png' border=1 width=200 />" >> $results
        echo "" >> $results
        echo '```' >> $results
        cat $outdir/result.out >> $results
        echo "" >> $results
        echo '```' >> $results
      fi
      echo "" >> $results

    done

  done
done
