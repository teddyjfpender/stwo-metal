#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"

lane = "generated-metal"
metal_label = "Metal generated"
summary_label = "Wide Fibonacci, Blake2s channel (SIMD vs Metal generated lane)"
paths = []

until ARGV.empty?
  case ARGV.first
  when "--lane"
    ARGV.shift
    lane = ARGV.shift or abort "missing value for --lane"
  when "--metal-label"
    ARGV.shift
    metal_label = ARGV.shift or abort "missing value for --metal-label"
  when "--summary"
    ARGV.shift
    summary_label = ARGV.shift or abort "missing value for --summary"
  else
    paths << ARGV.shift
  end
end

abort "usage: ruby scripts/render_wide_fibonacci_comparison_table.rb [--lane LANE] [--metal-label LABEL] [--summary TITLE] <benchmark.json> [<benchmark.json> ...]" if paths.empty?

SIMD_BASELINE = {
  16 => { "prove_ms" => 199.0, "throughput" => 429.0 },
  17 => { "prove_ms" => 267.0, "throughput" => 478.0 },
  18 => { "prove_ms" => 450.0, "throughput" => 671.0 },
  19 => { "prove_ms" => 757.0, "throughput" => 909.0 },
  20 => { "prove_ms" => 1390.0, "throughput" => 973.0 },
  21 => { "prove_ms" => 2670.0, "throughput" => 1127.0 },
  22 => { "prove_ms" => 5166.0, "throughput" => 870.0 },
  23 => { "prove_ms" => 11014.0, "throughput" => 783.0 }
}.freeze

def format_decimal(value)
  return "N/A" if value.nil?

  format("%.2f", value)
end

def format_speedup(numerator, denominator)
  return "N/A" if numerator.nil? || denominator.nil? || denominator.zero?

  format("%.2fx", numerator / denominator)
end

rows = paths.map do |path|
  data = JSON.parse(File.read(path))
  workload = data.fetch("workload")
  next unless data.fetch("benchmark_id") == "wide_fibonacci_prove_verify_v1"
  next unless workload.fetch("family") == "wide_fibonacci"
  next unless workload.fetch("operation") == "prove_verify"

  timings = data.fetch("timings")
  phase_summary = timings["phase_summary_ms"] || {}
  prove_summary = phase_summary["prove_ms"] || {}

  {
    "path" => path,
    "lane" => data["benchmark_lane"] || "unknown",
    "log_n_instances" => workload.fetch("log_n_instances"),
    "prove_ms" => prove_summary["mean"],
    "throughput" => timings["throughput_kelem_per_second"]
  }
end.compact

lane_rows = rows.select { |row| row["lane"] == lane }
grouped = lane_rows.group_by { |row| row["log_n_instances"] }

puts "<details>"
puts "<summary>#{summary_label}</summary>"
puts
puts "| Log(Size) | Prove SIMD ms | Prove #{metal_label} ms | Speedup | Thr SIMD (Kelem/s) | Thr #{metal_label} (Kelem/s) | Thr Speedup |"
puts "|-----------|----------------|----------------|---------|---------------------|---------------------|-------------|"

SIMD_BASELINE.keys.sort.each do |log_size|
  simd = SIMD_BASELINE.fetch(log_size)
  row = grouped.fetch(log_size, []).min_by { |candidate| candidate.fetch("prove_ms", Float::INFINITY) }
  metal_prove_ms = row&.fetch("prove_ms", nil)
  metal_throughput = row&.fetch("throughput", nil)

  puts [
    log_size,
    format_decimal(simd["prove_ms"]),
    format_decimal(metal_prove_ms),
    format_speedup(simd["prove_ms"], metal_prove_ms),
    format_decimal(simd["throughput"]),
    format_decimal(metal_throughput),
    format_speedup(metal_throughput, simd["throughput"])
  ].join(" | ").prepend("| ").concat(" |")
end

puts
puts "</details>"
