//! The house's wallet, owned by `HouseState`: its cash balance (House
//! `+0x30C`), the spending statistic (`+0x2DC`) and the ore feeder of its
//! score (`+0x54E8`). Every writer goes through the money primitives here,
//! each the one port of its native function. The house's silo storage
//! (`+0x2FC`) and capacity (`+0x310`) are not kept: their only filler is the
//! `Weeder=` deposit (`Add_Tiberium_To_Storage @ 0x004F9700`, called only at
//! `0x0073E48E`), which retail never sets, so the primitives' storage arms
//! never run (tools/house_money_oracle.py pins them on an empty store).
//!
//! Never depends on render/ui/audio/net (sim invariant #1). The wallet is
//! serialized and hashed.

use crate::rules::ruleset::INCOME_PPM_SCALE;

/// Per-house wallet and statistics. There is no second house credit balance.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Economy {
    /// House `+0x30C`, the cash balance.
    credits: i32,
    /// House `+0x2DC`: the running total `Spend_Money` took.
    spent_credits: i32,
    /// The ore feeder of House `+0x54E8`, written by `Add_Tiberium_Credits`;
    /// its kill and capture feeders are `MatchStatistics::score_points`.
    harvested_credits: i32,
}

impl Economy {
    /// A wallet holding `credits`: `HouseClass::Set_Credits_And_Color @
    /// 0x004FCE00` seeds the balance of a new house.
    pub(crate) const fn new(credits: i32) -> Self {
        Self {
            credits,
            spent_credits: 0,
            harvested_credits: 0,
        }
    }

    pub const fn credits(&self) -> i32 {
        self.credits
    }

    pub const fn spent_credits(&self) -> i32 {
        self.spent_credits
    }

    pub const fn harvested_credits(&self) -> i32 {
        self.harvested_credits
    }

    /// `HouseClass::Available_Money @ 0x004F6990`: `ftol(storage value *
    /// IncomeMult + balance)`, the balance while the silos are empty.
    pub const fn available_money(&self) -> i32 {
        self.credits
    }

    /// `HouseClass::Add_Credits @ 0x004F9950`: `balance += amount`, wrapping.
    pub(crate) fn add_credits(&mut self, amount: i32) {
        self.credits = self.credits.wrapping_add(amount);
    }

    /// `HouseClass::Spend_Money @ 0x004F9790`. An amount the balance covers
    /// (signed `<=`) comes off it; otherwise the whole balance, even a
    /// negative one, is taken and zeroed, and the rest would drain the silos
    /// (`0x004F97DD..`, empty here). What was taken joins `+0x2DC`.
    pub(crate) fn spend_money(&mut self, amount: i32) {
        let taken = if amount <= self.credits {
            self.credits = self.credits.wrapping_sub(amount);
            amount
        } else {
            std::mem::take(&mut self.credits)
        };
        self.spent_credits = self.spent_credits.wrapping_add(taken);
    }

    /// `HouseClass::Add_Tiberium_Credits @ 0x004F9610` for `amount` units
    /// of a tiberium: `score = ftol(amount * 5 + score)` and `balance =
    /// ftol(value * IncomeMult * amount + balance)`, `value` being
    /// the tiberium's `Value=`. VERA's amount is `bales * scale_ppm / 1e6`
    /// (the deposit passes 1.0, its purifier bonus `P * PurifierBonus`), its
    /// `value` the bales' summed worth and the IncomeMult `income_ppm / 1e6`;
    /// each sum truncates toward zero once, as the `ftol` does. Native runs
    /// in floats: an IncomeMult a float cannot hold, such as 0.9, pays one
    /// credit more here (the oracle's `IncomeMult 0.9` row). A sum past
    /// `i32` wraps where `ftol` gives `0x80000000`.
    pub(crate) fn add_tiberium_credits(
        &mut self,
        value: i32,
        bales: i32,
        scale_ppm: i64,
        income_ppm: i64,
    ) {
        let scale = i128::from(INCOME_PPM_SCALE);
        let score = (i128::from(self.harvested_credits) * scale
            + i128::from(bales) * i128::from(scale_ppm) * 5)
            / scale;
        let credits = (i128::from(self.credits) * scale * scale
            + i128::from(value) * i128::from(scale_ppm) * i128::from(income_ppm))
            / (scale * scale);
        self.harvested_credits = score as i32;
        self.credits = credits as i32;
    }

    /// A captured or authored balance for a test.
    #[cfg(test)]
    pub(crate) fn set_credits_for_test(&mut self, credits: i32) {
        self.credits = credits;
    }

    /// A captured or authored spending statistic for a test.
    #[cfg(test)]
    pub(crate) fn set_spent_for_test(&mut self, spent: i32) {
        self.spent_credits = spent;
    }

    /// A captured or authored score feeder for a test.
    #[cfg(test)]
    pub(crate) fn set_harvested_for_test(&mut self, harvested: i32) {
        self.harvested_credits = harvested;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn rows(section: &str) -> Vec<Value> {
        let data: Value =
            serde_json::from_str(crate::test_fixture::text("tools/house_money_oracle.json"))
                .unwrap();
        data[section].as_array().unwrap().clone()
    }

    fn int(value: &Value) -> i32 {
        i32::try_from(value.as_i64().unwrap()).unwrap()
    }

    #[test]
    fn spend_money_matches_the_original() {
        for row in rows("spend_money") {
            let mut wallet = Economy::new(int(&row["balance"]));
            wallet.set_spent_for_test(int(&row["spent"]));
            wallet.spend_money(int(&row["amount"]));
            let expected = &row["expected"];
            assert_eq!(
                (wallet.credits(), wallet.spent_credits()),
                (int(&expected["balance"]), int(&expected["spent"])),
                "{}",
                row["name"]
            );
        }
    }

    #[test]
    fn add_credits_matches_the_original() {
        for row in rows("add_credits") {
            let mut wallet = Economy::new(int(&row["balance"]));
            wallet.add_credits(int(&row["amount"]));
            assert_eq!(
                wallet.credits(),
                int(&row["expected"]["balance"]),
                "{}",
                row["name"]
            );
        }
    }

    #[test]
    fn add_tiberium_credits_matches_the_original() {
        for row in rows("add_tiberium_credits") {
            let mut wallet = Economy::new(int(&row["balance"]));
            wallet.set_harvested_for_test(int(&row["score"]));
            let bales = int(&row["bales"]);
            wallet.add_tiberium_credits(
                int(&row["value"]) * bales,
                bales,
                row["scale_ppm"].as_i64().unwrap(),
                row["income_ppm"].as_i64().unwrap(),
            );
            let expected = &row["expected"];
            let mut balance = int(&expected["balance"]);
            if row["name"] == "IncomeMult 0.9 is a float" {
                // The documented float residual: ftol(25 * 0.9f * 40) = 899.
                assert_eq!(balance, 899);
                balance = 900;
            }
            assert_eq!(
                (wallet.credits(), wallet.harvested_credits()),
                (balance, int(&expected["score"])),
                "{}",
                row["name"]
            );
        }
    }

    #[test]
    fn available_money_matches_the_original() {
        for row in rows("available_money") {
            let wallet = Economy::new(int(&row["balance"]));
            assert_eq!(
                wallet.available_money(),
                int(&row["expected"]),
                "{}",
                row["name"]
            );
        }
    }
}
