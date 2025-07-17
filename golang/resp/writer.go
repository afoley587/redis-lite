package resp

import (
	"io"
)

type RespWriter struct {
	writer io.Writer
}

func NewRespWriter(w io.Writer) *RespWriter {
	return &RespWriter{writer: w}
}

func (rw *RespWriter) Write(value RespValue) error {
	_, err := rw.writer.Write(value.Marshal())
	return err
}

func (rw *RespWriter) WriteOK() error {
	return rw.Write(NewSimpleString("OK"))
}

func (rw *RespWriter) WriteError(msg string) error {
	return rw.Write(NewError(msg))
}
