require "json"

metadata = JSON.parse(`cargo metadata --format-version 1 --frozen`)
packages = metadata["packages"]
puts JSON.pretty_generate(packages)
