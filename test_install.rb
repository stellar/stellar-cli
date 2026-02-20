require "open3"
require "shellwords"

class Matcher
  def initialize(string, expected)
    @string = string
    @expected = expected
  end

  def match?(out)
    out.include?(@string) == @expected
  end

  def inspect
    @expected ? "match #{@string.inspect}" : "not match #{@string.inspect}"
  end
end

$EXPECTED_EXIT = 0

DISTROS = {
  debian: "debian:bookworm-slim",
  ubuntu: "ubuntu:latest",
  fedora: "fedora:latest"
}

def run_cmd(*cmd, exit: 0)
  formatted_cmd = cmd.map { Shellwords.escape(it) }.join(" ")
  out, err, status = Open3.capture3(*cmd)

  if status.exitstatus != exit
    fail "Expected status to be #{exit}; " \
         "got #{status.exitstatus}\n\n" \
         "== command ==\n#{formatted_cmd}\n\n" \
         "== stdout ==\n#{format_output(out)}\n\n" \
         "== stderr ==\n#{format_output(err)}"
  end

  [out, err]
end

def run_docker(distro, cmd, exit: 0)
  run_cmd(
    "docker",
    "run",
    "--rm",
    "-v",
    "#{__dir__}:/source:ro",
    DISTROS.fetch(distro),
    "sh",
    "-c",
    cmd,
    exit:
  )
end

def match(matcher)
  Matcher.new(matcher, true)
end

def doesnt_match(matcher)
  Matcher.new(matcher, false)
end

def format_output(output)
  output = "\\n" if output == "\n"
  output = "<empty>" if output.empty?
  output
end

def run_test(title, distro:, cmd:, exit: 0, out: nil, err: nil)
  title = "\e[34m#{title}\e[0m"
  print "\e[33m[ACTING]\e[0m #{title}"

  stdout, stderr  = run_docker(distro, cmd, exit:)
  outputs = "\n\n== stdout ==\n#{format_output(stdout)}\n\n"
  outputs = "#{outputs}== stderr ==\n#{format_output(stderr)}"

  Array(out).each do |item|
    if item
      unless item.match?(stdout)
        fail "Expected output to #{item.inspect}#{outputs}"
      end
    end
  end

  Array(err).each do |item|
    if item
      unless item.match?(stderr)
        fail "Expected output to #{item.inspect}#{outputs}"
      end
    end
  end

  puts "\r\e[32m[PASSED] #{title}\e[0m"
rescue RuntimeError => error
  puts "\r\e[31m[FAILED]\e[0m #{title}\nError: #{error.message}\n\n"
  $EXPECTED_EXIT = 1
end

run_test "sanity: output capture works",
  distro: :debian,
  cmd: "echo hello_from_container",
  out: match("hello_from_container")

run_test "debian: exits when curl is missing",
  distro: :debian,
  cmd: "sh /source/install.sh",
  exit: 1,
  out: match("Error: Missing required command(s): curl")

run_test "debian: no sudo in suggestions when running as root",
  distro: :debian,
  cmd: "apt-get update && apt-get -y install curl; sh /source/install.sh",
  exit: 1,
  out: [doesnt_match("sudo apt-get"), doesnt_match("sudo dnf")]

run_test "fedora: no sudo in suggestions when running as root",
  distro: :fedora,
  cmd: "dnf -y install curl; sh /source/install.sh",
  exit: 1,
  out: [doesnt_match("sudo apt-get"), doesnt_match("sudo dnf")]

run_test "debian: sudo in suggestions when running as user",
  distro: :debian,
  cmd: <<~SH,
    apt-get update && apt-get -y install curl sudo
    adduser --disabled-password --gecos "" test
    su -c "sh /source/install.sh" test
  SH
  exit: 1,
  out: match("sudo apt-get")

run_test "fedora: sudo in suggestions when running as user",
  distro: :fedora,
  cmd: <<~SH,
    dnf -y install curl util-linux
    adduser test
    runuser -u test -- sh /source/install.sh
  SH
  exit: 1,
  out: match("sudo dnf")

run_test "debian: warns about missing runtime libraries",
  distro: :debian,
  cmd: <<~SH,
    apt-get update && apt-get -y install curl
    apt-get -y --allow-remove-essential remove util-linux libudev1 libapt-pkg6.0 apt
    sh /source/install.sh
  SH
  exit: 1,
  out: [
    match("runtime shared libraries are missing"),
    match("libdbus-1"),
    match("libudev")
  ]

run_test "fedora: warns about missing runtime libraries",
  distro: :fedora,
  cmd: "dnf -y install curl; rpm -e --nodeps systemd-libs; sh /source/install.sh",
  exit: 1,
  out: [
    match("runtime shared libraries are missing"),
    match("libdbus-1"),
    match("libudev")
  ]

run_test "debian: successful install",
  distro: :debian,
  cmd: "apt-get update && apt-get -y install curl libdbus-1-3 libudev1; sh /source/install.sh",
  out: [
    match("Stellar CLI installed successfully"),
    doesnt_match("runtime shared libraries are missing")
  ]

run_test "ubuntu: successful install",
  distro: :ubuntu,
  cmd: "apt-get update && apt-get -y install curl libdbus-1-3 libudev1; sh /source/install.sh",
  out: [
    match("Stellar CLI installed successfully"),
    doesnt_match("runtime shared libraries are missing")
  ]

run_test "fedora: successful install",
  distro: :fedora,
  cmd: "dnf -y install curl dbus-libs systemd-libs; sh /source/install.sh",
  out: [
    match("Stellar CLI installed successfully"),
    doesnt_match("runtime shared libraries are missing")
  ]

run_test "debian: install missing dependencies",
  distro: :debian,
  cmd: "apt-get update && apt-get -y install curl; sh /source/install.sh --install-deps",
  out: [
    match("Stellar CLI installed successfully"),
    match("[OK] rustup"),
    match("[OK] cargo"),
    match("[OK] rustc"),
    match("[OK] wasm32v1-none target"),
    doesnt_match("runtime shared libraries are missing")
  ]

run_test "ubuntu: install missing dependencies",
  distro: :ubuntu,
  cmd: "apt-get update && apt-get -y install curl; sh /source/install.sh --install-deps",
  out: [
    match("Stellar CLI installed successfully"),
    match("[OK] rustup"),
    match("[OK] cargo"),
    match("[OK] rustc"),
    match("[OK] wasm32v1-none target"),
    doesnt_match("runtime shared libraries are missing")
  ]

run_test "fedora: install missing dependencies",
  distro: :fedora,
  cmd: "dnf -y install curl; sh /source/install.sh --install-deps",
  out: [
    match("Stellar CLI installed successfully"),
    match("[OK] rustup"),
    match("[OK] cargo"),
    match("[OK] rustc"),
    match("[OK] wasm32v1-none target"),
    doesnt_match("runtime shared libraries are missing")
  ]

exit $EXPECTED_EXIT
