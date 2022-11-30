package config

var (
	// Version is the soroban-rpc version number, which is injected during build time.
	Version = "0.0.0"

	// CommitHash is the soroban-rpc git commit hash, which is injected during build time.
	CommitHash = ""

	// BuildTimestamp is the timestamp at which the soroban-rpc was built, injected during build time.
	BuildTimestamp = ""

	// Branch is the git branch from which the soroban-rpc was built, injected during build time.
	Branch = ""
)
