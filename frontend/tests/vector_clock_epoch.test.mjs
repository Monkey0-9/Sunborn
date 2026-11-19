import test from 'node:test';
import assert from 'node:assert/strict';
import { extractVectorClockEpoch } from '../src/lib/utils.ts';

test('extractVectorClockEpoch handles Rust VectorClock nested clocks object', () => {
  const payload = {
    clocks: {
      'gateway-primary-node-1': 1,
    },
  };
  const result = extractVectorClockEpoch(payload);
  assert.equal(typeof result, 'number');
  assert.equal(result, 1);
});

test('extractVectorClockEpoch handles multi-node Rust VectorClock taking max epoch', () => {
  const payload = {
    clocks: {
      'gateway-node-us-east': 12,
      'gateway-node-eu-west': 29,
      'gateway-node-ap-south': 17,
    },
  };
  const result = extractVectorClockEpoch(payload);
  assert.equal(typeof result, 'number');
  assert.equal(result, 29);
});

test('extractVectorClockEpoch handles flat map format', () => {
  const payload = {
    'gateway-node-1': 42,
  };
  const result = extractVectorClockEpoch(payload);
  assert.equal(typeof result, 'number');
  assert.equal(result, 42);
});

test('extractVectorClockEpoch handles direct epoch property', () => {
  const payload = { epoch: 888 };
  const result = extractVectorClockEpoch(payload);
  assert.equal(typeof result, 'number');
  assert.equal(result, 888);
});

test('extractVectorClockEpoch handles direct number', () => {
  assert.equal(extractVectorClockEpoch(10), 10);
});

test('extractVectorClockEpoch falls back safely on null, undefined, empty, or invalid input without throwing or returning an object', () => {
  assert.equal(extractVectorClockEpoch(null), 1);
  assert.equal(extractVectorClockEpoch(undefined), 1);
  assert.equal(extractVectorClockEpoch({}), 1);
  assert.equal(extractVectorClockEpoch({ clocks: {} }), 1);
  assert.equal(extractVectorClockEpoch({ clocks: { 'node-1': 'invalid' } }), 1);
  assert.equal(extractVectorClockEpoch(NaN), 1);
  assert.equal(extractVectorClockEpoch(null, 5), 5);
});
