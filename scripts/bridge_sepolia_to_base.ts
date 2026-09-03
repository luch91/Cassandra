import { createPublicClient, createWalletClient, http, parseEther, formatEther, encodeFunctionData } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { sepolia } from "viem/chains";
import { readFileSync } from "node:fs";

const BASE_SEPOLIA_CHAIN_ID = 84532;
const L1_STANDARD_BRIDGE = "0xfd0Bf71F60660E2f608ed56e1659C450eB113120" as const;
const SOURCE_RPC = process.env.ETH_SEPOLIA_RPC_URL ?? "https://ethereum-sepolia-rpc.publicnode.com";
const DEST_RPC = process.env.BASE_SEPOLIA_RPC_URL ?? "https://sepolia.base.org";
const walletFile = process.env.TELEGRAPH_WALLET_ENV ?? `${process.env.HOME}/.config/telegraph/base-sepolia-sentinel.env`;
const bridgeAbi = [{
  type: "function", name: "bridgeETHTo", stateMutability: "payable",
  inputs: [{ name: "_to", type: "address" }, { name: "_minGasLimit", type: "uint32" }, { name: "_extraData", type: "bytes" }], outputs: [],
}] as const;

function getArg(name: string): string | undefined {
  const i = process.argv.indexOf(name); return i >= 0 ? process.argv[i + 1] : undefined;
}
function loadPrivateKey(): `0x${string}` {
  const line = readFileSync(walletFile, "utf8").split("\n").find((x: string) => x.startsWith("EVM_PRIVATE_KEY="));
  const key = line?.slice("EVM_PRIVATE_KEY=".length).trim();
  if (!key?.match(/^0x[0-9a-fA-F]{64}$/)) throw new Error(`Invalid or missing EVM_PRIVATE_KEY in ${walletFile}`);
  return key as `0x${string}`;
}

async function main() {
  if (!process.argv.includes("--confirm")) throw new Error("Refusing bridge: --confirm is required.");
  const amountText = getArg("--amount");
  if (!amountText) throw new Error("Usage: npm run bridge:sepolia-to-base -- --amount 0.02 --confirm");
  const amount = parseEther(amountText);
  if (amount <= 0n) throw new Error("Amount must be positive.");
  const account = privateKeyToAccount(loadPrivateKey());
  const depositData = encodeFunctionData({ abi: bridgeAbi, functionName: "bridgeETHTo", args: [account.address, 200000, "0x"] });
  const source = createPublicClient({ chain: sepolia, transport: http(SOURCE_RPC) });
  const destination = createPublicClient({ chain: { ...sepolia, id: BASE_SEPOLIA_CHAIN_ID }, transport: http(DEST_RPC) });
  const [sourceChain, sourceBalance, bridgeCode] = await Promise.all([
    source.getChainId(), source.getBalance({ address: account.address }), source.getBytecode({ address: L1_STANDARD_BRIDGE }),
  ]);
  if (sourceChain !== 11155111) throw new Error(`Wrong source chain: ${sourceChain}`);
  if (!bridgeCode || bridgeCode === "0x") throw new Error("L1StandardBridge has no bytecode at the expected Sepolia address.");
  const gas = await source.estimateGas({ account, to: L1_STANDARD_BRIDGE, value: amount, data: depositData });
  const fees = await source.estimateFeesPerGas();
  const gasCost = gas * (fees.maxFeePerGas ?? fees.gasPrice ?? 0n);
  if (sourceBalance < amount + gasCost) throw new Error(`Insufficient source balance: ${formatEther(sourceBalance)} ETH; need at least ${formatEther(amount + gasCost)} ETH including estimated gas.`);
  const wallet = createWalletClient({ account, chain: sepolia, transport: http(SOURCE_RPC) });
  console.log(JSON.stringify({ source_chain: sourceChain, destination_chain: BASE_SEPOLIA_CHAIN_ID, bridge: L1_STANDARD_BRIDGE, sender: account.address, recipient: account.address, amount_eth: amountText, estimated_gas: gas.toString(), estimated_max_fee_eth: formatEther(gasCost) }));
  if (process.argv.includes("--dry-run")) { console.log("dry_run: no transaction submitted"); return; }
  const hash = await wallet.writeContract({ address: L1_STANDARD_BRIDGE, abi: bridgeAbi, functionName: "bridgeETHTo", args: [account.address, 200000, "0x"], value: amount });
  console.log(JSON.stringify({ submitted_tx: hash, explorer: `https://sepolia.etherscan.io/tx/${hash}` }));
  const receipt = await source.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`L1 bridge transaction reverted: ${hash}`);
  const destinationBefore = await destination.getBalance({ address: account.address });
  console.log(JSON.stringify({ l1_status: receipt.status, l1_block: receipt.blockNumber.toString(), destination_balance_at_check_eth: formatEther(destinationBefore), note: "L2 relay may still be pending; poll this address on Base Sepolia." }));
}
main().catch((e: Error) => { console.error(e.message); process.exitCode = 1; });
