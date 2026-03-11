#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"

abort "usage: ruby scripts/check_benchmark_class_acceptance.rb <primary|supporting|report> <benchmark.json>" if ARGV.length != 2

mode = ARGV.fetch(0)
path = ARGV.fetch(1)

unless %w[primary supporting report].include?(mode)
  abort "invalid mode #{mode.inspect}; expected primary, supporting, or report"
end

data = JSON.parse(File.read(path))
runner = data.fetch("runner")
workload = data.fetch("workload")
timings = data.fetch("timings")
summary = timings["steady_state_summary_ms"] || timings["summary_ms"]

errors = []
errors << "status is not completed" unless data["status"] == "completed"
errors << "runner_class is not benchmark-gpu" unless runner["runner_class"] == "benchmark-gpu"
errors << "warmup_iterations < 1" unless workload.fetch("warmup_iterations", 0) >= 1
errors << "sample_iterations < 5" unless workload.fetch("sample_iterations", 0) >= 5

if summary.nil?
  errors << "summary_ms is missing"
else
  mean = summary.fetch("mean")
  median = summary.fetch("median")
  stddev = summary.fetch("stddev")
  max = summary.fetch("max")

  errors << "mean must be positive" unless mean.is_a?(Numeric) && mean.positive?
  errors << "median must be positive" unless median.is_a?(Numeric) && median.positive?
  errors << "stddev must be non-negative" unless stddev.is_a?(Numeric) && stddev >= 0.0
  errors << "max must be positive" unless max.is_a?(Numeric) && max.positive?
end

stability_reasons = []
if summary
  mean = summary["mean"].to_f
  median = summary["median"].to_f
  stddev = summary["stddev"].to_f
  max = summary["max"].to_f

  if mean > 0.0
    stability_reasons << format("stddev/mean=%.4f exceeds 0.0500", stddev / mean) if (stddev / mean) > 0.05
  end

  if median > 0.0
    stability_reasons << format("max/median=%.4f exceeds 1.1000", max / median) if (max / median) > 1.10
  end
end

primary_ok = errors.empty? && stability_reasons.empty?
supporting_ok = errors.empty? && !stability_reasons.empty?

puts "benchmark: #{data.fetch("benchmark_id")}"
puts "workload: #{workload.fetch("family")}/#{workload.fetch("channel")}/#{workload.fetch("operation")}"
puts "runner_class: #{runner.fetch("runner_class")}"
puts "status: #{data.fetch("status")}"
puts "summary_source: #{timings.key?("steady_state_summary_ms") ? "steady_state_summary_ms" : "summary_ms"}"
puts "primary_ok: #{primary_ok}"
puts "supporting_ok: #{supporting_ok}"
unless errors.empty?
  puts "errors:"
  errors.each { |error| puts "  - #{error}" }
end
unless stability_reasons.empty?
  puts "stability_reasons:"
  stability_reasons.each { |reason| puts "  - #{reason}" }
end

case mode
when "primary"
  exit(primary_ok ? 0 : 1)
when "supporting"
  exit(supporting_ok ? 0 : 1)
else
  exit 0
end
