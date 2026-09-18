'use strict'

const native = require('./index.js')

function parse(result) {
  return JSON.parse(result)
}

exports.convertFile = async (path, options = undefined) =>
  parse(native.convertFile(path, options ? JSON.stringify(options) : undefined))

exports.convertBuffer = async (buffer, options = undefined) =>
  parse(native.convertBuffer(buffer, options ? JSON.stringify(options) : undefined))

exports.validateOutput = async (path, options = undefined) =>
  parse(native.validateOutput(path, options ? JSON.stringify(options) : undefined))

exports.inspectFile = async path => parse(native.inspectFile(path))
exports.inspectBuffer = async buffer => parse(native.inspectBuffer(buffer))
exports.explain = native.explain
exports.runtimeSource = native.runtimeSource
