package config

import "fmt"

type LogFormat int

const (
	LogFormatText LogFormat = iota
	LogFormatJSON
)

func (f LogFormat) MarshalText() ([]byte, error) {
	switch f {
	case LogFormatText:
		return []byte("text"), nil
	case LogFormatJSON:
		return []byte("json"), nil
	default:
		return nil, fmt.Errorf("unknown log format: %d", f)
	}
}

func (f *LogFormat) UnmarshalText(text []byte) error {
	switch string(text) {
	case "text":
		*f = LogFormatText
	case "json":
		*f = LogFormatJSON
	default:
		return fmt.Errorf("unknown log format: %s", text)
	}
	return nil
}

func (f LogFormat) MarshalTOML() ([]byte, error) {
	return f.MarshalText()
}

func (f *LogFormat) UnmarshalTOML(i interface{}) error {
	switch v := i.(type) {
	case []byte:
		return f.UnmarshalText(v)
	case string:
		return f.UnmarshalText([]byte(v))
	default:
		return fmt.Errorf("unknown log format: %v", v)
	}
}

func (f LogFormat) String() string {
	switch f {
	case LogFormatText:
		return "text"
	case LogFormatJSON:
		return "json"
	default:
		panic(fmt.Sprintf("unknown log format: %d", f))
	}
}
