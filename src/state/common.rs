// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/common.ts

use super::merkle_tree::MerklePath;
use super::types::{hash, shl, Fr};
use ff::{Field, PrimeField, PrimeFieldRepr};

#[derive(Clone, Copy)]
pub struct Order {
    pub status: Fr,
    pub tokenbuy: Fr,
    pub tokensell: Fr,
    pub filled_sell: Fr,
    pub filled_buy: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
}

impl Order {
    pub fn empty() -> Self {
        Self {
            status: Fr::one(),
            tokenbuy: Fr::zero(),
            tokensell: Fr::zero(),
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            total_sell: Fr::zero(),
            total_buy: Fr::zero(),
        }
    }
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();
        data.add_assign(&self.status);
        data.add_assign(&shl(&self.tokenbuy, 32));
        data.add_assign(&shl(&self.tokensell, 64));
        hash(&[data, self.filled_sell, self.filled_buy, self.total_sell, self.total_buy])
    }
}

#[derive(Copy, Clone)]
pub struct AccountState {
    pub nonce: Fr,
    pub sign: Fr,
    pub balanceRoot: Fr,
    pub ay: Fr,
    pub ethAddr: Fr,
    pub orderRoot: Fr,
}

impl AccountState {
    pub fn empty(balanceRoot: Fr, orderRoot: Fr) -> Self {
        Self {
            nonce: Fr::zero(),
            sign: Fr::zero(),
            balanceRoot,
            ay: Fr::zero(),
            ethAddr: Fr::zero(),
            orderRoot,
        }
    }
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();

        data.add_assign(&self.nonce);
        data.add_assign(&shl(&self.sign, 40));
        let inputs = &[data, self.balanceRoot, self.ay, self.ethAddr, self.orderRoot];
        hash(inputs)
    }
    // TODO: combine with emptyAccount
    pub fn new() -> Self {
        Self {
            nonce: Fr::zero(),
            sign: Fr::zero(),
            balanceRoot: Fr::zero(),
            ay: Fr::zero(),
            ethAddr: Fr::zero(),
            orderRoot: Fr::zero(),
        }
    }
    /*
    pub fn updateAccountKey(account) {
      const sign = BigInt(account.sign);
      const ay = Scalar.fromString(account.ay, 16);
      const ethAddr = Scalar.fromString(account.ethAddr.replace('0x', ''), 16);
      self.updateL2Addr(sign, ay, ethAddr);
    }
    */
    // TODO: remove ethAddr
    pub fn updateL2Addr(&mut self, sign: Fr, ay: Fr, ethAddr: Fr) {
        self.sign = sign;
        self.ay = ay;
        self.ethAddr = ethAddr;
    }
    pub fn updateNonce(&mut self, nonce: Fr) {
        self.nonce = nonce;
    }
    pub fn updateOrderRoot(&mut self, orderRoot: Fr) {
        self.orderRoot = orderRoot;
    }
}

#[derive(Copy, Clone)]
pub enum TxType {
    DepositToNew,
    DepositToOld,
    Transfer,
    Withdraw,
    PlaceOrder,
    SpotTrade,
    Nop,
}

pub const TxLength: usize = 34;
pub mod TxDetailIdx {
    pub const TokenID: usize = 0;
    pub const Amount: usize = 1;
    pub const AccountID1: usize = 2;
    pub const AccountID2: usize = 3;
    pub const EthAddr1: usize = 4;
    pub const EthAddr2: usize = 5;
    pub const Sign1: usize = 6;
    pub const Sign2: usize = 7;
    pub const Ay1: usize = 8;
    pub const Ay2: usize = 9;
    pub const Nonce1: usize = 10;
    pub const Nonce2: usize = 11;
    pub const Balance1: usize = 12;
    pub const Balance2: usize = 13;
    pub const Balance3: usize = 14;
    pub const Balance4: usize = 15;
    pub const SigL2Hash: usize = 16;
    pub const S: usize = 17;
    pub const R8x: usize = 18;
    pub const R8y: usize = 19;

    // only used in spot_trade
    pub const TokenID2: usize = 20;
    pub const Amount2: usize = 21;
    pub const Order1ID: usize = 22;
    pub const Order1AmountSell: usize = 23;
    pub const Order1AmountBuy: usize = 24;
    pub const Order1FilledSell: usize = 25;
    pub const Order1FilledBuy: usize = 26;
    pub const Order2ID: usize = 27;
    pub const Order2AmountSell: usize = 28;
    pub const Order2AmountBuy: usize = 29;
    pub const Order2FilledSell: usize = 30;
    pub const Order2FilledBuy: usize = 31;

    // only used in place_order
    pub const TokenID3: usize = 32;
    pub const TokenID4: usize = 33;
}

pub struct RawTx {
    pub txType: TxType,
    pub payload: Vec<Fr>,
    pub balancePath0: MerklePath,
    pub balancePath1: MerklePath,
    pub balancePath2: MerklePath,
    pub balancePath3: MerklePath,
    pub orderPath0: MerklePath,
    pub orderPath1: MerklePath,
    pub orderRoot0: Fr,
    pub orderRoot1: Fr,
    pub accountPath0: MerklePath,
    pub accountPath1: MerklePath,
    pub rootBefore: Fr,
    pub rootAfter: Fr,
    // debug info
    // extra: any;
}

pub struct L2Block {
    pub txsType: Vec<TxType>,
    pub encodedTxs: Vec<Vec<Fr>>,
    pub balance_path_elements: Vec<[MerklePath; 4]>,
    pub order_path_elements: Vec<[MerklePath; 2]>,
    pub account_path_elements: Vec<[MerklePath; 2]>,
    pub orderRoots: Vec<[Fr; 2]>,
    pub oldAccountRoots: Vec<Fr>,
    pub newAccountRoots: Vec<Fr>,
}

// TODO: remove previous_...
pub struct PlaceOrderTx {
    pub accountID: u32,
    pub previous_tokenID_sell: u32,
    pub previous_tokenID_buy: u32,
    pub previous_amount_sell: Fr,
    pub previous_amount_buy: Fr,
    pub previous_filled_sell: Fr,
    pub previous_filled_buy: Fr,
    pub tokenID_sell: u32,
    pub tokenID_buy: u32,
    pub amount_sell: Fr,
    pub amount_buy: Fr,
}

pub struct DepositToOldTx {
    pub accountID: u32,
    pub tokenID: u32,
    pub amount: Fr,
}

pub struct SpotTradeTx {
    pub order1_accountID: u32,
    pub order2_accountID: u32,
    pub tokenID_1to2: u32,
    pub tokenID_2to1: u32,
    pub amount_1to2: Fr,
    pub amount_2to1: Fr,
    pub order1_id: u32,
    pub order2_id: u32,
}
