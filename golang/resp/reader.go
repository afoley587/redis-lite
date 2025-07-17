package resp

import (
	"fmt"
	"strconv"
)

type Resp struct {
	Values []RespValue // Parsed top-level values
	curr   int
	buf    []byte
}

func NewResp(buf []byte) *Resp {
	return &Resp{
		buf:    buf,
		Values: make([]RespValue, 0),
	}
}

func (r *Resp) Read() (RespValue, error) {
	if r.curr >= len(r.buf) {
		return RespValue{}, fmt.Errorf("empty buffer")
	}

	switch r.readByte() {
	case RespString:
		return r.readSimpleString(), nil
	case RespError:
		return r.readError(), nil
	case RespInteger:
		return r.readInteger()
	case RespBulk:
		return r.readBulk()
	case RespArray:
		return r.readArray()
	case RespNull:
		return NewNull(), nil
	default:
		return RespValue{}, fmt.Errorf("unknown RESP type at byte: %d", r.curr-1)
	}
}

func (r *Resp) HasNext() bool {
	// Skip trailing whitespace like \r or \n if any
	for r.curr < len(r.buf) {
		if r.buf[r.curr] != '\r' && r.buf[r.curr] != '\n' {
			return true
		}
		r.curr++
	}
	return false
}

func (r *Resp) readSimpleString() RespValue {
	str := r.readLine()
	return NewSimpleString(string(str))
}

func (r *Resp) readError() RespValue {
	msg := r.readLine()
	return NewError(string(msg))
}

func (r *Resp) readInteger() (RespValue, error) {
	line := r.readLine()
	num, err := strconv.Atoi(string(line))
	if err != nil {
		return RespValue{}, fmt.Errorf("invalid integer: %v", err)
	}
	return NewInteger(num), nil
}

func (r *Resp) readBulk() (RespValue, error) {
	lengthLine := r.readLine()
	length, err := strconv.Atoi(string(lengthLine))
	if err != nil {
		return RespValue{}, fmt.Errorf("invalid bulk string length: %v", err)
	}

	if length == -1 {
		return NewNull(), nil
	}

	start := r.curr
	end := start + length

	if end > len(r.buf) {
		return RespValue{}, fmt.Errorf("bulk string out of bounds")
	}

	bulk := string(r.buf[start:end])
	r.curr = end + 2 // skip CRLF

	return NewBulkString(bulk), nil
}

func (r *Resp) readArray() (RespValue, error) {
	lengthLine := r.readLine()
	length, err := strconv.Atoi(string(lengthLine))
	if err != nil {
		return RespValue{}, fmt.Errorf("invalid array length: %v", err)
	}

	if length == -1 {
		return NewNull(), nil
	}

	values := make([]RespValue, 0, length)
	for i := 0; i < length; i++ {
		val, err := r.Read()
		if err != nil {
			return RespValue{}, fmt.Errorf("array item %d: %v", i, err)
		}
		values = append(values, val)
	}

	return NewArray(values), nil
}

func (r *Resp) readLine() []byte {
	start := r.curr
	for {
		if r.curr+1 >= len(r.buf) || (r.buf[r.curr] == '\r' && r.buf[r.curr+1] == '\n') {
			break
		}
		r.curr++
	}
	end := r.curr
	r.curr += 2 // skip CRLF
	return r.buf[start:end]
}

func (r *Resp) readByte() byte {
	b := r.buf[r.curr]
	r.curr++
	return b
}
