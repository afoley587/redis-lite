package resp

import "strconv"

const (
	RespString  = '+'
	RespError   = '-'
	RespInteger = ':'
	RespBulk    = '$'
	RespArray   = '*'
	RespNull    = '_'
)

type RespValue struct {
	Type    byte
	String  string
	Integer int
	Bulk    string
	Array   []RespValue
}

// Factory methods for constructing RESP values
func NewSimpleString(s string) RespValue {
	return RespValue{Type: RespString, String: s}
}

func NewBulkString(b string) RespValue {
	return RespValue{Type: RespBulk, Bulk: b}
}

func NewInteger(i int) RespValue {
	return RespValue{Type: RespInteger, Integer: i}
}

func NewArray(arr []RespValue) RespValue {
	return RespValue{Type: RespArray, Array: arr}
}

func NewNull() RespValue {
	return RespValue{Type: RespNull}
}

func NewError(msg string) RespValue {
	return RespValue{Type: RespError, String: msg}
}

func (rv RespValue) Marshal() []byte {
	switch rv.Type {
	case RespString:
		return marshalSimpleString(rv.String)
	case RespError:
		return marshalError(rv.String)
	case RespInteger:
		return marshalInteger(rv.Integer)
	case RespBulk:
		return marshalBulk(rv.Bulk)
	case RespArray:
		return marshalArray(rv.Array)
	case RespNull:
		return marshalNull()
	default:
		return []byte{}
	}
}

func marshalSimpleString(s string) []byte {
	return append([]byte{RespString}, append([]byte(s), '\r', '\n')...)
}

func marshalError(msg string) []byte {
	return append([]byte{RespError}, append([]byte(msg), '\r', '\n')...)
}

func marshalInteger(i int) []byte {
	return append([]byte{RespInteger}, append([]byte(strconv.Itoa(i)), '\r', '\n')...)
}

func marshalBulk(b string) []byte {
	return append(
		append(
			append([]byte{RespBulk}, []byte(strconv.Itoa(len(b)))...),
			'\r', '\n'),
		append([]byte(b), '\r', '\n')...,
	)
}

func marshalArray(arr []RespValue) []byte {
	out := []byte{RespArray}
	out = append(out, []byte(strconv.Itoa(len(arr)))...)
	out = append(out, '\r', '\n')

	for _, v := range arr {
		out = append(out, v.Marshal()...)
	}
	return out
}

func marshalNull() []byte {
	return []byte{RespNull, '\r', '\n'}
}
