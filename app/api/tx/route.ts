import {
  address, createSolanaRpc,
  createTransactionMessage, setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstruction,
  compileTransaction, getBase64EncodedWireTransaction,
  lamports,
} from "@solana/web3.js";
import { getTransferSolInstruction } from '@solana-program/system';

// standard headers for this route (including CORS)
const headers = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET,POST,PUT,OPTIONS",
  "Access-Control-Allow-Headers":
    "Content-Type, Authorization, " +
    "Content-Encoding, Accept-Encoding, " +
    "X-Accept-Action-Version, X-Accept-Blockchain-Ids",
  "Access-Control-Expose-Headers": "X-Action-Version, X-Blockchain-Ids",
  "Content-Type": "application/json",
};

// amount we are transacting with here
const lamportCount = lamports(BigInt(process.env.TX_LAMPORTS || 1));
const LAMPORTS_PER_SOL = 1_000_000_000;

export async function GET(req: Request) {
  try {
    const payload = {
      type: 'action',
      label: process.env.TX_LABEL || "Solana Transaction",
      icon: "https://solana.com/src/img/branding/solanaLogoMark.svg",
      title: process.env.TX_SOURCE || "My place",
      description: "Send " + lamportCount + "/" + LAMPORTS_PER_SOL + " SOL",
    };

    return Response.json(payload, { headers });
  } catch (error) {
    const text = 'Error identifying recipient is ' + error;
    console.error(text);
    const options = { ...headers, status: 500, statusText: text }
    return new Response(text, options);
  }
}

export const OPTIONS = async () => Response.json(null, { headers });

export async function POST(req: Request) {
  try {
    const DEVNET_RPC = "https://api.devnet.solana.com"
    const data = await req.json();
    console.log("POST Recieved", data);

    const payerAddressStr = data.account;
    const payerAddress = address(payerAddressStr);
    console.log("Payer address", payerAddress);

    const recipientAddressStr = process.env.TX_RECIPIENT_ACCOUNT;
    const recipientAddress = address(recipientAddressStr);
    console.log("Recipent address", recipientAddress);

    console.log("Lamports", lamportCount);

    const transfer = getTransferSolInstruction({
      source: payerAddress,
      destination: recipientAddress,
      amount: lamportCount,
    });
    console.log("Instruction", transfer);

    const rpcEndpoint = process.env.TX_RPC_ENDPOINT || DEVNET_RPC;
    console.log("Endpoint is", rpcEndpoint);

    const rpc = createSolanaRpc(rpcEndpoint);
    console.log("Have", rpc, "for endpoint", rpcEndpoint);

    const { value: latestBlockhash } =
      await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    console.log("Latest hash", latestBlockhash);

    let txMsg = createTransactionMessage({ version: 0 });
    txMsg = setTransactionMessageFeePayer(recipientAddress, txMsg);
    txMsg = appendTransactionMessageInstruction(transfer, txMsg);
    txMsg = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, txMsg);
    console.log("TransactionMessage", txMsg);

    const transaction = compileTransaction(txMsg as any);
    console.log("Compiled transaction");

    const base64Tx = getBase64EncodedWireTransaction(transaction);
    console.log("base64Tx:", base64Tx);
    const txPurpose =
      `Transfer ${lamportCount} lamports to ${recipientAddress}`;

    return Response.json({ transaction: base64Tx, message: txPurpose },
      { headers });
  } catch (error) {
    const text = `Error initiating transaction is ${error}`
    console.error(text);
    console.log(error.stack);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

