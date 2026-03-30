import Testing
import Foundation
@testable import Snowed

@Suite("Canonical Decimal String")
struct CanonicalDecimalTests {

    @Test func integer() {
        #expect(canonicalDecimalString(Decimal(1))   == "1")
        #expect(canonicalDecimalString(Decimal(0))   == "0")
        #expect(canonicalDecimalString(Decimal(-42)) == "-42")
    }

    @Test func trailingZerosStripped() {
        #expect(canonicalDecimalString(Decimal(string: "1.0")!)   == "1")
        #expect(canonicalDecimalString(Decimal(string: "1.00")!)  == "1")
        #expect(canonicalDecimalString(Decimal(string: "1.50")!)  == "1.5")
        #expect(canonicalDecimalString(Decimal(string: "1.500")!) == "1.5")
    }

    @Test func fractional() {
        #expect(canonicalDecimalString(Decimal(string: "3.14159")!) == "3.14159")
        #expect(canonicalDecimalString(Decimal(string: "0.001")!)   == "0.001")
        #expect(canonicalDecimalString(Decimal(string: "-0.5")!)    == "-0.5")
    }

    @Test func leadingZeroPreserved() {
        #expect(canonicalDecimalString(Decimal(string: "0.5")!) == "0.5")
    }

    @Test func negativeZeroNormalized() {
        #expect(canonicalDecimalString(Decimal(0)) == "0")
    }

    @Test func largeInteger() {
        #expect(canonicalDecimalString(Decimal(string: "1000000")!) == "1000000")
    }

    @Test func highPrecision() {
        #expect(canonicalDecimalString(Decimal(string: "123456789.123456789")!) == "123456789.123456789")
    }
}
