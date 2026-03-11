#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"

abort "usage: ruby scripts/render_wide_fibonacci_dual_lane_report.rb <generated_dir> <generic_dir>" if ARGV.length != 2

generated_dir, generic_dir = ARGV

def timing_summary(timings, steady_key, default_key)
  timings[steady_key] || timings[default_key] || {}
end

def load_rows(dir)
  Dir[File.join(dir, "wide_fibonacci_prove_log*.json")].sort.map do |path|
    data = JSON.parse(File.read(path))
    workload = data.fetch("workload")
    timings = data.fetch("timings")
    phase_summary = timing_summary(timings, "steady_state_phase_summary_ms", "phase_summary_ms")
    prove_summary = phase_summary["prove_ms"] || {}
    throughput =
      timings["steady_state_throughput_kelem_per_second"] ||
      timings["throughput_kelem_per_second"]
    cold_start_phase = timings["cold_start_phase_ms"] || {}

    {
      "lane" => data["benchmark_lane"] || "unknown",
      "log_n_instances" => workload.fetch("log_n_instances"),
      "prove_ms" => prove_summary["mean"],
      "throughput" => throughput,
      "cold_start_prove_ms" => cold_start_phase["prove_ms"]
    }
  end
end

def format_logs(rows)
  logs = rows.map { |row| row.fetch("log_n_instances") }.sort
  return "none" if logs.empty?
  return logs.first.to_s if logs.length == 1

  contiguous = logs.each_cons(2).all? { |left, right| right == left + 1 }
  contiguous ? "#{logs.first}..#{logs.last}" : logs.join(", ")
end

def format_decimal(value)
  return "N/A" if value.nil?

  format("%.2f", value)
end

generated_rows = load_rows(generated_dir)
generic_rows = load_rows(generic_dir)

generated_log20 = generated_rows.find { |row| row["log_n_instances"] == 20 }
generic_first = generic_rows.min_by { |row| row["log_n_instances"] }

generated_table = File.read(File.join(generated_dir, "wide_fibonacci_comparison.md"))
generic_table = File.read(File.join(generic_dir, "wide_fibonacci_comparison.md"))

puts "# Wide Fibonacci Dual-Lane Report"
puts
puts "- Generated lane range: `#{format_logs(generated_rows)}`"
puts "- Generic lane range: `#{format_logs(generic_rows)}`"
puts "- Generated `log_size = 20` prove ms: `#{format_decimal(generated_log20&.fetch("prove_ms", nil))}`"
puts "- Generated `log_size = 20` cold-start prove ms: `#{format_decimal(generated_log20&.fetch("cold_start_prove_ms", nil))}`"
puts "- Generic first-row prove ms: `#{format_decimal(generic_first&.fetch("prove_ms", nil))}` at `log_size = #{generic_first&.fetch("log_n_instances", "N/A")}`"
puts "- Generic first-row cold-start prove ms: `#{format_decimal(generic_first&.fetch("cold_start_prove_ms", nil))}`"
puts "- Active optimization target: `generated-metal`"
puts
puts "## Generated Lane"
puts
puts generated_table
puts
puts "## Generic Lane"
puts
puts generic_table
