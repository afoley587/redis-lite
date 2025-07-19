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
