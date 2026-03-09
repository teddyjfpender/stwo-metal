#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"

abort "usage: ruby scripts/render_benchmark_table.rb <benchmark.json> [<benchmark.json> ...]" if ARGV.empty?

def format_number(value)
  return "N/A" if value.nil?

  format("%.3f", value)
end

rows = ARGV.map do |path|
  data = JSON.parse(File.read(path))
  workload = data.fetch("workload")
  timings = data.fetch("timings")
  summary = timings["summary_ms"] || {}
  phase_summary = timings["phase_summary_ms"] || {}
  prove_summary = phase_summary["prove_ms"] || {}
  verify_summary = phase_summary["verify_ms"] || {}

  {
    "workload" => "#{workload.fetch("family")}/#{workload.fetch("operation")}",
    "channel" => workload.fetch("channel"),
    "log_n_instances" => workload.fetch("log_n_instances"),
    "n_columns" => workload.fetch("n_columns"),
    "mean_ms" => summary["mean"],
    "prove_mean_ms" => prove_summary["mean"],
    "verify_mean_ms" => verify_summary["mean"],
    "median_ms" => summary["median"],
    "min_ms" => summary["min"],
    "max_ms" => summary["max"],
    "throughput_kelem_per_second" => timings["throughput_kelem_per_second"],
    "classification" => data.fetch("classification"),
    "git_commit" => data.fetch("git_commit")[0, 12]
  }
end

puts "| Workload | Channel | Log(Size) | Columns | Mean ms | Prove ms | Verify ms | Median ms | Min ms | Max ms | Thr Kelem/s | Classification | Commit |"
puts "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
rows.each do |row|
  puts [
    row["workload"],
    row["channel"],
    row["log_n_instances"],
    row["n_columns"],
    format_number(row["mean_ms"]),
    format_number(row["prove_mean_ms"]),
    format_number(row["verify_mean_ms"]),
    format_number(row["median_ms"]),
    format_number(row["min_ms"]),
    format_number(row["max_ms"]),
    format_number(row["throughput_kelem_per_second"]),
    row["classification"],
    row["git_commit"]
  ].join(" | ").prepend("| ").concat(" |")
end
